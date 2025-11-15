use std::borrow::Borrow;
use std::{fs, io, slice};
use std::io::{BufWriter, Cursor, Seek, Write};
use std::ops::{Index, IndexMut};
use std::path::Path;
use binrw::{binrw, BinRead, BinWriterExt};
use directxtex::{HResultError, Image, ScratchImage, CP_FLAGS, CP_FLAGS_NONE, DDS_FLAGS, DDS_FLAGS_NONE, DXGI_FORMAT_BC6H_SF16, DXGI_FORMAT_BC6H_UF16, DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_COMPRESS_DEFAULT, TEX_THRESHOLD_DEFAULT};
use crate::convert;
use crate::image::helpers;

#[cfg(feature = "image")]
use image::{ColorType, DynamicImage, ExtendedColorType};

#[derive(Debug, thiserror::Error)]
pub enum BoxReflectionError {
    #[error("Io error")]
    IoError(#[from] io::Error),

    #[error("Parsing error")]
    ParsingError(#[from] binrw::Error),

    #[error("Error building boxreflections: {0}")]
    PackingError(String),

    #[error("DirectxTex error {0}")]
    DirectXTexError(#[from] HResultError),

    #[error("Error {0}")]
    Other(String),
}

#[binrw]
#[derive(Default, Clone, Debug)]
pub struct BoxReflectionCollection{
    #[br(temp)]
    #[bw(calc(entries.len() as u32))]
    num_entries: u32,
    #[br(count = num_entries)]
    pub(crate) entries: Vec<BoxReflection>,
}

impl BoxReflectionCollection {
    pub fn num_box_reflections(&self) -> usize {
        self.entries.len()
    }

    pub fn box_reflections(&self) -> &Vec<BoxReflection> {
        &self.entries
    }

    pub fn box_reflections_mut(&mut self) -> &mut Vec<BoxReflection> {
        &mut self.entries
    }

    pub fn box_reflection(&self, index: usize) -> Option<&BoxReflection> {
        self.entries.get(index)
    }

    pub fn box_reflection_mut(&mut self, index: usize) -> Option<&mut BoxReflection> {
        self.entries.get_mut(index)
    }

    fn nearest_by<I, T>(iter: I, x: f32, y: f32, z: f32) -> Option<I::Item>
    where
        I: Iterator<Item = T>,
        T: Borrow<BoxReflection>,
    {
        iter.min_by(|a, b| {
            let a_ref = a.borrow();
            let b_ref = b.borrow();

            let dx_a = a_ref.x() - x;
            let dy_a = a_ref.y() - y;
            let dz_a = a_ref.z() - z;
            let da = dx_a * dx_a + dy_a * dy_a + dz_a * dz_a;

            let dx_b = b_ref.x() - x;
            let dy_b = b_ref.y() - y;
            let dz_b = b_ref.z() - z;
            let db = dx_b * dx_b + dy_b * dy_b + dz_b * dz_b;

            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn box_reflection_at(&self, x: f32, y: f32, z: f32) -> Option<&BoxReflection> {
        Self::nearest_by(self.entries.iter(), x, y, z)
    }

    pub fn box_reflection_at_mut(&mut self, x: f32, y: f32, z: f32) -> Option<&mut BoxReflection> {
        Self::nearest_by(self.entries.iter_mut(), x, y, z)
    }
}

#[binrw]
#[derive(Default, Clone, Debug)]
pub struct BoxReflection {
    pos: [f32; 3],
    #[br(temp)]
    #[bw(calc(buffer.len() as u32))]
    size: u32,
    #[br(count = size)]
    pub(crate) buffer: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum CubemapLayout {
    HorizontalStrip,
    VerticalStrip,
    HorizontalCross,
    VerticalCross,
}

impl CubemapLayout{
    pub fn variants() -> [Self; 4] {
        [
            Self::HorizontalStrip,
            Self::VerticalStrip,
            Self::HorizontalCross,
            Self::VerticalCross,
        ]
    }

    pub fn from_tile_counts(width: usize, height: usize) -> Option<Self> {
        let dims = (width, height);
        Self::variants()
            .iter()
            .copied()
            .find(|v| v.tile_counts() == dims)
    }
    pub fn tile_positions(&self) -> [(usize, usize);6]{
        match self {
            //   +X -X +Y -Y +Z -Z
            CubemapLayout::HorizontalStrip => [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
            CubemapLayout::VerticalStrip => [(0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5)],
            //   .  +Y  .
            //   -X +Z  +X  -Z
            //   .  -Y  .
            CubemapLayout::HorizontalCross => [(2, 1), (0, 1), (1, 0), (1, 2), (1, 1), (3, 1)],
            //   .  +Y  .
            //   -X +Z +X
            //   .  -Y  .
            //   .  Z-  .
            CubemapLayout::VerticalCross => [(2, 1), (0, 1), (1, 0), (1, 2), (1, 1), (1, 3)],
        }
    }

    pub fn tile_counts(&self) -> (usize, usize){
        let face_tile_positions = self.tile_positions();
        let num_width_tiles = face_tile_positions.iter().map(|(w, _)| *w).max().unwrap_or_default() + 1;
        let num_height_tiles = face_tile_positions.iter().map(|(_, h)| *h).max().unwrap_or_default() + 1;
        (num_width_tiles, num_height_tiles)
    }
}

impl BoxReflection {
    pub fn x(&self) -> f32 { self.pos[2] }
    pub fn y(&self) -> f32 { self.pos[1] }
    pub fn z(&self) -> f32 { self.pos[0] }

    pub fn tile_width() -> usize {
        128
    }
    pub fn tile_height() -> usize {
        128
    }

    pub fn width(layout: CubemapLayout) -> usize {
        layout.tile_counts().0
    }

    #[cfg(feature = "image")]
    pub fn from_dynamic_image(image: &DynamicImage, pos: [f32; 3]) -> Result<Self, BoxReflectionError> {
        let extended_color = match &image.color(){
            ColorType::L8 => ExtendedColorType::L8,
            ColorType::La8 => ExtendedColorType::La8,
            ColorType::Rgb8 => ExtendedColorType::Rgb8,
            ColorType::Rgba8 => ExtendedColorType::Rgba8,
            ColorType::L16 => ExtendedColorType::L16,
            ColorType::La16 => ExtendedColorType::La16,
            ColorType::Rgb16 => ExtendedColorType::Rgb16,
            ColorType::Rgba16 => ExtendedColorType::Rgba16,
            ColorType::Rgb32F => ExtendedColorType::Rgb32F,
            ColorType::Rgba32F => ExtendedColorType::Rgba32F,
            _ => return Err(BoxReflectionError::Other("Cannot find dynamic image".to_owned()))
        };

        let scratch_image = helpers::dynamic_image_to_scratch_image(image.as_bytes(), image.width(), image.height(), extended_color).map_err(|e| BoxReflectionError::Other(e.to_string()))?;
        Self::from_scratch_image(scratch_image, pos)
    }

    pub fn from_dds(data: Vec<u8>, pos: [f32; 3]) -> Result<BoxReflection, BoxReflectionError>{
        let dds = ScratchImage::load_dds(&data, DDS_FLAGS_NONE, None, None)?;
        Self::from_scratch_image(dds, pos)
    }

    pub fn from_scratch_image(scratch_image: ScratchImage, pos: [f32; 3]) -> Result<BoxReflection, BoxReflectionError> {

        let (w, h) = (scratch_image.metadata().width, scratch_image.metadata().height);

        let cols = w / Self::tile_width();
        let rows = h / Self::tile_height();

        let layout = CubemapLayout::from_tile_counts(cols, rows);

        if let Some(layout) = layout {
            let scratch = cubemap_utils::decompose_layout(scratch_image.image(0,0,0).unwrap(), layout)?;
            let image = cubemap_utils::compose_layout(&scratch, CubemapLayout::HorizontalStrip)?;
            let compressed = image.compress(DXGI_FORMAT_BC6H_SF16, TEX_COMPRESS_DEFAULT, TEX_THRESHOLD_DEFAULT)?;
            let image = compressed.image(0,0,0).unwrap();
            let buffer = convert::image_pixels(image).unwrap_or_default();
            Ok(Self { pos, buffer })
        } else {
            Err(BoxReflectionError::Other(
                "Couldn't parse image format a boxreflection should use 128x128 faces:\n\
                    Vertical strip:   (1x6) = 128x768\n\
                    Horizontal strip: (6x1) = 768x128\n\
                    Horizontal cross: (4x3) = 512x384\n\
                    Vertical cross:   (3x4) = 384x512\n\
                refer to https://github.com/Microsoft/DirectXTex/wiki/Texassemble for more info".into(),
            ))
        }
    }

    pub fn create_dds(&mut self, layout: Option<CubemapLayout>) -> Result<Vec<u8>, BoxReflectionError> {
        let cubemap = self.create_cubemap_image(true)?;
        let scratch = match layout {
            None => {cubemap}
            Some(layout) => {
               cubemap_utils::compose_layout(&cubemap, layout)?
            }
        };

        let blob = scratch
            .save_dds(DDS_FLAGS::DDS_FLAGS_NONE)
            .map_err(BoxReflectionError::DirectXTexError)?;

        let bytes = blob.buffer();
        Ok(Vec::from(bytes))
    }
    fn create_cubemap_image(&mut self, decompressed: bool) -> Result<ScratchImage, BoxReflectionError> {

        let pitch = DXGI_FORMAT_BC6H_UF16
                    .compute_pitch(Self::tile_width(), Self::tile_height(), CP_FLAGS::CP_FLAGS_NONE)
                    .map_err(BoxReflectionError::DirectXTexError)?;

        let face_size = pitch.slice;
        let total_needed = face_size.checked_mul(6).unwrap();
        if self.buffer.len() < total_needed {
            println!("buffer too small for 6 faces")
        }

        let base_ptr = self.buffer.as_mut_ptr();

        let images: Vec<(Vec<u8>, Image)> = (0..6).map(|face| {
            let ptr = unsafe { base_ptr.add(face * face_size) };
            let mut out = unsafe { slice::from_raw_parts(ptr, pitch.slice) }.to_vec();
            let img = Image {
                width: Self::tile_width(),
                height: Self::tile_height(),
                format: DXGI_FORMAT_BC6H_UF16,
                row_pitch: pitch.row,
                slice_pitch: pitch.slice,
                pixels: out.as_mut_ptr(),
            };
            (out, img)
        }).collect();

        let (buffers, faces_array): (Vec<Vec<u8>>, Vec<Image>) = images.into_iter().unzip();
        let _buffers = buffers; // keeps allocations alive until end of scope  TODO: Remove this hack
        let mut scratch_image = ScratchImage::default();
        scratch_image.initialize_cube_from_images(faces_array.as_slice(), CP_FLAGS_NONE)?;

        if decompressed {
            scratch_image = scratch_image.decompress(DXGI_FORMAT_R16G16B16A16_FLOAT)?;
        }
        Ok(scratch_image)
    }
}

impl BoxReflectionCollection{
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, BoxReflectionError> {
        let data = fs::read(path).map_err(BoxReflectionError::IoError)?;
        Self::new_inner(&data)
    }

    pub fn from_memory(data: &[u8]) -> Result<Self, BoxReflectionError> {
        Self::new_inner(data)
    }

    fn new_inner(data: &[u8]) -> Result<Self, BoxReflectionError>{
        let mut stream = Cursor::new(data);
        BoxReflectionCollection::read_le(&mut stream).map_err(BoxReflectionError::ParsingError)
    }

    pub fn pack_to_vec(&self) -> Result<Vec<u8>, BoxReflectionError> {
        let mut writer = Cursor::new(Vec::new());
        self.pack_internal(&mut writer)?;
        Ok(writer.into_inner())
    }

    pub fn pack_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), BoxReflectionError> {
        let file = fs::File::create(path).map_err(BoxReflectionError::IoError)?;
        let mut writer = BufWriter::new(file);
        self.pack_internal(&mut writer)?;
        Ok(())
    }

    fn pack_internal<W: Write + Seek>(&self, writer: &mut W) -> Result<(), BoxReflectionError> {
        writer.write_le(self).map_err(|e| BoxReflectionError::PackingError(format!("Unable to pack boxreflections: {e}")))?;
        Ok(())
    }

    pub fn push(&mut self, br: BoxReflection) {
        self.entries.push(br)
    }

    pub fn insert(&mut self, index: usize, br: BoxReflection) {
        self.entries.insert(index, br)
    }

    pub fn remove(&mut self, index: usize) -> Option<BoxReflection> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear()
    }

    pub fn iter(&self) -> impl Iterator<Item = &BoxReflection> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut BoxReflection> {
        self.entries.iter_mut()
    }
}

impl Index<usize> for BoxReflectionCollection {
    type Output = BoxReflection;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for BoxReflectionCollection {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl<'a> IntoIterator for &'a BoxReflectionCollection {
    type Item = &'a BoxReflection;
    type IntoIter = std::slice::Iter<'a, BoxReflection>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a mut BoxReflectionCollection {
    type Item = &'a mut BoxReflection;
    type IntoIter = std::slice::IterMut<'a, BoxReflection>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut()
    }
}

pub mod cubemap_utils {
    use std::ptr::NonNull;
    use std::{slice};
    use directxtex::{Rect, ScratchImage, CP_FLAGS_NONE, DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_FILTER_FLAGS};
    use super::{Image, CubemapLayout, BoxReflectionError, BoxReflection};

    fn rotate180(image: &Image) -> Result<(), BoxReflectionError> {
        let pixels_ptr = NonNull::new(image.pixels).unwrap();
        let scanlines = image.format.compute_scanlines(image.height);
        let len = image.row_pitch.checked_mul(scanlines).unwrap();
        let pixels = unsafe { slice::from_raw_parts_mut(pixels_ptr.as_ptr(), len) };

        let pixel_stride: usize = image.format.bits_per_pixel() / 8;
        let mut left = 0usize;
        let mut right = len - pixel_stride;
        while left < right {
            for k in 0..pixel_stride {
                pixels.swap(left + k, right + k);
            }
            left += pixel_stride;
            right = right.saturating_sub(pixel_stride);
        }
        Ok(())
    }

    pub fn compose_layout(images: &ScratchImage, layout: CubemapLayout) -> Result<ScratchImage, BoxReflectionError> {

        let face_w = BoxReflection::tile_width();
        let face_h = BoxReflection::tile_height();
        let bytes_per_pixel: usize = images.metadata().format.bits_per_pixel() / 8;

        let face_tile_positions = layout.tile_positions();
        let num_width_tiles = face_tile_positions.iter().map(|(w, _)| *w).max().unwrap_or_default() + 1;
        let num_height_tiles = face_tile_positions.iter().map(|(_, h)| *h).max().unwrap_or_default() + 1;
        let final_w = num_width_tiles * face_w;
        let final_h = num_height_tiles * face_h;
        let final_row_pitch = final_w * bytes_per_pixel;
        let final_slice_pitch = final_row_pitch * final_h;

        let mut out: Vec<u8> = vec![0u8; final_slice_pitch];
        let mut image = Image {
            width: final_w,
            height: final_h,
            format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            row_pitch: final_row_pitch,
            slice_pitch: final_slice_pitch,
            pixels: out.as_mut_ptr(),
        };

        for (face_index,(tile_x, tile_y)) in face_tile_positions.iter().enumerate() {
            let face_image = images.image(0, face_index, 0)
                .ok_or(BoxReflectionError::Other("Failed to find cubemap image".into()))?;
            if matches!(layout, CubemapLayout::VerticalCross) && face_index == 5 {
                rotate180(face_image)?
            }
            let rect = Rect { x: 0, y: 0, w: face_w, h: face_h, };
            image.copy_rectangle(face_image, &rect, TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT, tile_x * face_w, tile_y * face_h)?;
        }
        let mut scratch_image = ScratchImage::default();
        scratch_image.initialize_from_image(&image, false, CP_FLAGS_NONE)?;
        Ok(scratch_image)
    }

    pub fn decompose_layout(image: &Image, layout: CubemapLayout) -> Result<ScratchImage, BoxReflectionError> {
        let face_w = BoxReflection::tile_width();
        let face_h = BoxReflection::tile_height();

        let bytes_per_pixel: usize = 8;

        let face_tile_positions = layout.tile_positions();

        let num_w_tiles = face_tile_positions.iter().map(|(w, _)| *w).max().unwrap_or_default() + 1;
        let num_h_tiles = face_tile_positions.iter().map(|(_, h)| *h).max().unwrap_or_default() + 1;
        let expected_w = num_w_tiles * face_w;
        let expected_h = num_h_tiles * face_h;

        if image.width < expected_w || image.height < expected_h {
            return Err(BoxReflectionError::Other(format!(
                "Input image too small for requested layout: got {}x{}, need {}x{}",
                image.width, image.height, expected_w, expected_h
            )));
        }

        let mut faces_vec: Vec<(Vec<u8>, Image)> = Vec::with_capacity(6);

        for (face_index, (tile_x, tile_y)) in face_tile_positions.iter().enumerate() {

            let final_row_pitch = face_w * bytes_per_pixel;
            let final_slice_pitch = final_row_pitch * face_h;

            let mut out: Vec<u8> = vec![0u8; final_slice_pitch];
            let mut face_image = Image {
                width: face_w,
                height: face_h,
                format: DXGI_FORMAT_R16G16B16A16_FLOAT,
                row_pitch: final_row_pitch,
                slice_pitch: final_slice_pitch,
                pixels: out.as_mut_ptr(),
            };

            let rect = Rect { x: tile_x * face_w, y: tile_y * face_h, w: face_w, h: face_h, };
            face_image.copy_rectangle(image, &rect, TEX_FILTER_DEFAULT, 0, 0)?;

            if matches!(layout, CubemapLayout::VerticalCross) && face_index == 5 {
                rotate180(&face_image)?;
            }
            faces_vec.push((out, face_image));
        }

        let (buffers, faces_array): (Vec<Vec<u8>>, Vec<Image>) = faces_vec.into_iter().unzip();
        let _buffers = buffers; // keeps allocations alive until end of scope TODO: Remove this hack
        let mut scratch_image = ScratchImage::default();
        scratch_image.initialize_cube_from_images(faces_array.as_slice(), CP_FLAGS_NONE)?;
        Ok(scratch_image)
    }
}