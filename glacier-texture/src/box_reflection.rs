use std::borrow::Borrow;
use std::{fs, io};
use std::io::{BufWriter, Cursor, Seek, Write};
use std::path::Path;
use binrw::{binrw, BinRead, BinWriterExt};
use directxtex::{HResultError, Image, ScratchImage, CP_FLAGS, DDS_FLAGS, DDS_FLAGS_NONE, DXGI_FORMAT_BC6H_SF16, DXGI_FORMAT_BC6H_UF16, DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_COMPRESS_DEFAULT, TEX_THRESHOLD_DEFAULT};
use crate::convert;

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
    pub num_entries: u32,
    #[br(count = num_entries)]
    pub entries: Vec<BoxReflection>,
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
    buffer: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum CubemapLayout {
    HorizontalStrip,
    VerticalStrip,
    HorizontalCross,
    VerticalCross,
}

impl BoxReflection {
    pub fn x(&self) -> f32 { self.pos[2] }
    pub fn y(&self) -> f32 { self.pos[1] }
    pub fn z(&self) -> f32 { self.pos[0] }

    pub fn width() -> usize {
        128
    }
    pub fn height() -> usize {
        128
    }

    pub fn from_dds(data: Vec<u8>, pos: [f32; 3]) -> Result<BoxReflection, BoxReflectionError> {
        let dds = ScratchImage::load_dds(&data, DDS_FLAGS_NONE, None, None)?;

        let (w, h) = (dds.metadata().width, dds.metadata().height);

        let cols = w / Self::width();
        let rows = h / Self::height();

        let layout = match (cols, rows) {
            (1, 6) => Some(CubemapLayout::VerticalStrip),
            (6, 1) => Some(CubemapLayout::HorizontalStrip),
            (4, 3) => Some(CubemapLayout::HorizontalCross),
            (3, 4) => Some(CubemapLayout::VerticalCross),
            _ => None,
        };

        if let Some(layout) = layout {
            let scratch = cubemap_utils::decompose_layout(dds.image(0,0,0).unwrap(), layout)?;
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
                let image = cubemap_utils::compose_layout(&cubemap, layout)?;

                let mut out_scratch = ScratchImage::default();
                out_scratch
                    .initialize_from_image(&image, false, CP_FLAGS::CP_FLAGS_NONE)
                    .map_err(BoxReflectionError::DirectXTexError)?;
                out_scratch
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
                    .compute_pitch(Self::width(), Self::height(), CP_FLAGS::CP_FLAGS_NONE)
                    .map_err(BoxReflectionError::DirectXTexError)?;

        let face_size = pitch.slice;
        let total_needed = face_size.checked_mul(6).unwrap();
        if self.buffer.len() < total_needed {
            println!("buffer too small for 6 faces")
        }

        let base_ptr = self.buffer.as_mut_ptr();

        let images: Vec<Image> = (0..6).map(|face| {
            let ptr = unsafe { base_ptr.add(face * face_size) };
            Image {
                width: Self::width(),
                height: Self::height(),
                format: DXGI_FORMAT_BC6H_UF16,
                row_pitch: pitch.row,
                slice_pitch: pitch.slice,
                pixels: ptr,
            }
        }).collect();
        let mut image = ScratchImage::default();
        image.initialize_cube_from_images(images.as_slice(), CP_FLAGS::CP_FLAGS_NONE)?;
        if decompressed {
            image = image.decompress(DXGI_FORMAT_R16G16B16A16_FLOAT)?;
        }
        Ok(image)
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
}

pub mod cubemap_utils {
    use std::ptr::NonNull;
    use std::{slice};
    use directxtex::{Rect, ScratchImage, CP_FLAGS, DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_FILTER_FLAGS};
    use super::{Image, CubemapLayout, BoxReflectionError, BoxReflection};

    fn rotate180(image: &Image) -> Result<(), BoxReflectionError> {
        let pixels_ptr = NonNull::new(image.pixels).unwrap();
        let scanlines = image.format.compute_scanlines(image.height);
        let len = image.row_pitch.checked_mul(scanlines).unwrap();
        let pixels = unsafe {slice::from_raw_parts_mut(pixels_ptr.as_ptr(), len) };

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

    pub fn layout_positions(layout: CubemapLayout) -> [(usize,usize);6]{
        match &layout {
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
    pub fn compose_layout(images: &ScratchImage, layout: CubemapLayout) -> Result<Image, BoxReflectionError> {

        let face_w = BoxReflection::width();
        let face_h = BoxReflection::height();
        let bytes_per_pixel: usize = images.metadata().format.bits_per_pixel() / 8;

        let face_tile_positions = layout_positions(layout);
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
        Ok(image)
    }

    pub fn decompose_layout(image: &Image, layout: CubemapLayout) -> Result<ScratchImage, BoxReflectionError> {
        let face_w = BoxReflection::width();
        let face_h = BoxReflection::height();

        let bytes_per_pixel: usize = 8;

        let face_tile_positions = layout_positions(layout);

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

        let mut faces_vec: Vec<Image> = Vec::with_capacity(6);

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
            faces_vec.push(face_image);
        }

        let faces_array: [Image; 6] = faces_vec
            .try_into()
            .map_err(|_| BoxReflectionError::Other("Failed to assemble faces array".into()))?;

        let mut scratch = ScratchImage::default();
        scratch.initialize_cube_from_images(&faces_array, CP_FLAGS::CP_FLAGS_NONE)?;
        Ok(scratch)
    }
}