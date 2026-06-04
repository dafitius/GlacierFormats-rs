use std::borrow::Borrow;
use std::{fs, io, slice};
use std::io::{BufWriter, Cursor, Seek, Write};
use std::ops::{Index, IndexMut};
use std::path::Path;
use binrw::{binrw, BinRead, BinWriterExt};
use directxtex::{HResultError, Image, ScratchImage, CP_FLAGS, CP_FLAGS_NONE, DDS_FLAGS, DDS_FLAGS_NONE, DXGI_FORMAT_BC6H_UF16, DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_COMPRESS_DEFAULT, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT};
use crate::convert;
use crate::image::helpers;

#[cfg(feature = "image")]
use image::{ColorType, DynamicImage, ExtendedColorType};
use image::{ImageBuffer, Rgba, Rgba32FImage};
use glacier_base::math::Vector3;
pub use cubemap_utils::Orientation;

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
pub struct BoxReflectionCache {
    #[br(temp)]
    #[bw(calc(entries.len() as u32))]
    num_entries: u32,
    #[br(count = num_entries)]
    entries: Vec<BoxReflection>,
}

impl BoxReflectionCache {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn as_slice(&self) -> &[BoxReflection]{
        &self.entries
    }

    pub fn as_mut_slice(&mut self) -> &mut [BoxReflection] {
        &mut self.entries
    }

    pub fn get(&self, index: usize) -> Option<&BoxReflection> {
        self.entries.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut BoxReflection> {
        self.entries.get_mut(index)
    }

    fn nearest_by<I, T>(iter: I, position: Vector3) -> Option<I::Item>
    where
        I: Iterator<Item = T>,
        T: Borrow<BoxReflection>,
    {
        iter.min_by(|a, b| {
            let a_ref = a.borrow();
            let b_ref = b.borrow();

            let dx_a = a_ref.x() - position.x;
            let dy_a = a_ref.y() - position.y;
            let dz_a = a_ref.z() - position.z;
            let da = dx_a * dx_a + dy_a * dy_a + dz_a * dz_a;

            let dx_b = b_ref.x() - position.x;
            let dy_b = b_ref.y() - position.y;
            let dz_b = b_ref.z() - position.z;
            let db = dx_b * dx_b + dy_b * dy_b + dz_b * dz_b;

            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn get_at_position(&self, position: Vector3) -> Option<&BoxReflection> {
        Self::nearest_by(self.entries.iter(), position)
    }

    pub fn get_at_position_mut(&mut self, position: Vector3) -> Option<&mut BoxReflection> {
        Self::nearest_by(self.entries.iter_mut(), position)
    }
}



#[binrw]
#[derive(Default, Clone, Debug)]
pub struct BoxReflection {
    pos: Vector3,
    #[br(temp)]
    #[bw(calc(buffer.len() as u32))]
    size: u32,
    #[br(count = size)]
    buffer: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn from_tile_counts(tiles_x: usize, tiles_y: usize) -> Option<Self> {
        let dims = (tiles_x, tiles_y);
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
    #[allow(clippy::misnamed_getters)]
    pub fn x(&self) -> f32 { self.pos.z }// This is supposed to return z
    pub fn y(&self) -> f32 { self.pos.y }
    #[allow(clippy::misnamed_getters)]
    pub fn z(&self) -> f32 { self.pos.x }// This is supposed to return x

    pub const fn tile_width() -> usize {
        128
    }
    pub const fn tile_height() -> usize {
        128
    }

    pub(crate) fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    #[cfg(feature = "image")]
    pub fn from_dynamic_image(image: &DynamicImage, pos: Vector3) -> Result<Self, BoxReflectionError> {
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

    pub fn from_dds(data: Vec<u8>, pos: Vector3) -> Result<BoxReflection, BoxReflectionError>{
        let dds = ScratchImage::load_dds(&data, DDS_FLAGS_NONE, None, None)?;
        Self::from_scratch_image(dds, pos)
    }

    pub(crate) fn from_scratch_image(scratch_image: ScratchImage, pos: Vector3) -> Result<BoxReflection, BoxReflectionError> {

        let (w, h) = (scratch_image.metadata().width, scratch_image.metadata().height);

        let cols = w / Self::tile_width();
        let rows = h / Self::tile_height();

        let layout = CubemapLayout::from_tile_counts(cols, rows);

        if let Some(layout) = layout {
            let scratch_image = scratch_image.convert(DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT)?;
            let scratch = cubemap_utils::decompose_layout(scratch_image.image(0,0,0).unwrap(), layout)?;
            let image = cubemap_utils::compose_layout(&scratch, CubemapLayout::VerticalStrip)?;
            let compressed = image.compress(DXGI_FORMAT_BC6H_UF16, TEX_COMPRESS_DEFAULT, TEX_THRESHOLD_DEFAULT)?;
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

    pub fn create_dds(&self, layout: Option<CubemapLayout>) -> Result<Vec<u8>, BoxReflectionError> {
        self.create_dds_with_rotation(layout, [None, None, None])
    }

    pub fn create_dds_with_rotation(&self, layout: Option<CubemapLayout>, rotation: [Option<Orientation>; 3]) -> Result<Vec<u8>, BoxReflectionError> {
        let cubemap = self.create_cubemap_image(true)?;
        let scratch = match layout {
            None => {cubemap}
            Some(layout) => {
               cubemap_utils::compose_layout_with_rotation(&cubemap, layout, rotation)?
            }
        };

        let blob = scratch
            .save_dds(DDS_FLAGS::DDS_FLAGS_NONE)
            .map_err(BoxReflectionError::DirectXTexError)?;

        let bytes = blob.buffer();
        Ok(Vec::from(bytes))
    }

    #[cfg(feature = "image")]
    pub fn create_dynamic_image(
        &self,
        layout: CubemapLayout,
    ) -> Result<DynamicImage, BoxReflectionError> {
        self.create_dynamic_image_with_rotation(layout, [None, None, None])
    }

    #[cfg(feature = "image")]
    pub fn create_dynamic_image_with_rotation(
        &self,
        layout: CubemapLayout,
        rotation: [Option<Orientation>; 3],
    ) -> Result<DynamicImage, BoxReflectionError> {
        let cubemap = self.create_cubemap_image(true)?;
        let scratch = cubemap_utils::compose_layout_with_rotation(
            &cubemap,
            layout,
            rotation,
        )?;

        let metadata = scratch.metadata();
        let width = metadata.width;
        let height = metadata.height;

        let bytes = scratch.pixels();

        let expected_len = width * height * 4 * 2;
        if bytes.len() != expected_len as usize {
            return Err(BoxReflectionError::Other(
                "Failed to parse texture to image format".to_string(),
            ));
        }

        let data: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                half::f16::from_bits(bits).to_f32()
            })
            .collect();

        let img = Rgba32FImage::from_raw(width as u32, height as u32, data)
            .ok_or_else(|| BoxReflectionError::Other("Invalid image texture".to_string()))?;

        Ok(DynamicImage::ImageRgba32F(img))
    }

    fn create_cubemap_image(&self, decompressed: bool) -> Result<ScratchImage, BoxReflectionError> {

        let pitch = DXGI_FORMAT_BC6H_UF16
                    .compute_pitch(Self::tile_width(), Self::tile_height(), CP_FLAGS::CP_FLAGS_NONE)
                    .map_err(BoxReflectionError::DirectXTexError)?;

        let face_size = pitch.slice;
        let base_ptr = self.buffer.as_ptr();

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

impl BoxReflectionCache {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, BoxReflectionError> {
        let data = fs::read(path).map_err(BoxReflectionError::IoError)?;
        Self::new_inner(&data)
    }

    pub fn from_memory(data: &[u8]) -> Result<Self, BoxReflectionError> {
        Self::new_inner(data)
    }

    fn new_inner(data: &[u8]) -> Result<Self, BoxReflectionError>{
        let mut stream = Cursor::new(data);
        BoxReflectionCache::read_le(&mut stream).map_err(BoxReflectionError::ParsingError)
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

    pub fn remove(&mut self, index: usize) -> BoxReflection {
        self.entries.remove(index)
    }
    pub fn try_remove(&mut self, index: usize) -> Option<BoxReflection> {
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

impl Index<usize> for BoxReflectionCache {
    type Output = BoxReflection;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for BoxReflectionCache {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl<'a> IntoIterator for &'a BoxReflectionCache {
    type Item = &'a BoxReflection;
    type IntoIter = slice::Iter<'a, BoxReflection>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a mut BoxReflectionCache {
    type Item = &'a mut BoxReflection;
    type IntoIter = slice::IterMut<'a, BoxReflection>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut()
    }
}

impl<'a> FromIterator<&'a BoxReflection> for BoxReflectionCache
where
    BoxReflection: Clone,
{
    fn from_iter<T: IntoIterator<Item = &'a BoxReflection>>(iter: T) -> Self {
        let entries = iter
            .into_iter()
            .map(|b| (*b).clone()) // clone the owned BoxReflection, not the reference
            .collect::<Vec<BoxReflection>>();
        Self { entries }
    }
}

impl FromIterator<BoxReflection> for BoxReflectionCache {
    fn from_iter<T: IntoIterator<Item = BoxReflection>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}


mod cubemap_utils {
    use bitfield_struct::bitfield;
    use directxtex::{Rect, ScratchImage, CP_FLAGS_NONE, DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_FILTER_FLAGS};
    use crate::box_reflection::cubemap_utils::Orientation::{Rotate180, Rotate270, Rotate90};
    use super::{Image, CubemapLayout, BoxReflectionError, BoxReflection};


    #[derive(Copy, Clone, Debug)]
    pub enum Orientation {
        Rotate90,
        Rotate180,
        Rotate270,
    }

    #[bitfield(u8)]
    struct Flip {
        horizontal: bool,
        vertical: bool,
        #[bits(6)]
        _rem: u8,
    }

    pub fn rotate_image(image: &Image, rotate: Option<Orientation>) {
        use std::{ptr, slice};

        let pixel_stride = image.format.bits_per_pixel() / 8;
        let w = image.width;
        let h = image.height;
        let src_row_pitch = image.row_pitch;

        let dst_row_pitch = w * pixel_stride;
        let dst_len = dst_row_pitch * h;

        let src_ptr = image.pixels;
        let src_slice = unsafe { slice::from_raw_parts(src_ptr as *const u8, src_row_pitch * h) };

        let mut dst = vec![0u8; dst_len];
        let dst_ptr = dst.as_mut_ptr();

        let rot = rotate.map(|r| r.to_deg()).unwrap_or(0);

        let map = |x: usize, y: usize| -> (usize, usize) {
            match rot {
                0 => (x, y),
                90 => (h - 1 - y, x),
                180 => (w - 1 - x, h - 1 - y),
                270 => (y, w - 1 - x),
                _ => (x, y),
            }
        };

        for y in 0..h {
            let src_row_off = y * src_row_pitch;
            for x in 0..w {
                let src_off = src_row_off + x * pixel_stride;
                let (nx, ny) = map(x, y);
                let dst_off = ny * dst_row_pitch + nx * pixel_stride;
                unsafe {
                    ptr::copy_nonoverlapping(
                        src_slice.as_ptr().add(src_off),
                        dst_ptr.add(dst_off),
                        pixel_stride,
                    );
                }
            }
        }

        unsafe {
            ptr::copy_nonoverlapping(dst_ptr, src_ptr, dst_len);
        }
    }

    pub(crate) fn compose_layout(images: &ScratchImage, layout: CubemapLayout) -> Result<ScratchImage, BoxReflectionError> {
        compose_layout_with_rotation(images, layout, [None, None, None])
    }

    pub(crate) fn compose_layout_with_rotation(images: &ScratchImage, layout: CubemapLayout, rotation: [Option<Orientation>; 3]) -> Result<ScratchImage, BoxReflectionError> {

        if images.metadata().format != DXGI_FORMAT_R16G16B16A16_FLOAT {
            return Err(BoxReflectionError::Other(format!("Invalid format ({:?}), the Image format must be 4-channel half-float", images.metadata().format)))
        }

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

        for face_index in 0..6 {
            let face_image = images.image(0, face_index, 0)
                .ok_or(BoxReflectionError::Other("Failed to find cubemap image".into()))?;

            let mut rotation_steps = vec![];
            rotation_steps.push((Axis::Z, Rotate90)); //Adding this default rotation step to adjust for the standard rotation used by IOI.
            if let Some(x_rot) = rotation[0]{ rotation_steps.push((Axis::X, x_rot)); }
            if let Some(y_rot) = rotation[1]{ rotation_steps.push((Axis::Y, y_rot)); }
            if let Some(z_rot) = rotation[2]{ rotation_steps.push((Axis::Z, z_rot)); }
            let face_mapping = map_face_and_image_rotations(face_index, rotation_steps);

            if let (Some(new_face_idx), rotation) = face_mapping {
                rotate_image(face_image, rotation);
                let (tile_x, tile_y) = face_tile_positions[new_face_idx];

                if matches!(layout, CubemapLayout::VerticalCross) && new_face_idx == 5 {
                    rotate_image(face_image, Some(Rotate180));
                }

                let rect = Rect { x: 0, y: 0, w: face_w, h: face_h, };
                image.copy_rectangle(face_image, &rect, TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT, tile_x * face_w, tile_y * face_h)?;
            }
        }
        let mut scratch_image = ScratchImage::default();
        scratch_image.initialize_from_image(&image, false, CP_FLAGS_NONE)?;
        Ok(scratch_image)
    }

    pub(crate) fn decompose_layout(image: &Image, layout: CubemapLayout) -> Result<ScratchImage, BoxReflectionError> {
        let face_w = BoxReflection::tile_width();
        let face_h = BoxReflection::tile_height();

        if image.format != DXGI_FORMAT_R16G16B16A16_FLOAT {
            return Err(BoxReflectionError::Other(format!("Invalid format ({:?}), the Image format must be 4-channel half-float", image.format)))
        }

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
                rotate_image(&face_image, Some(Rotate180));
            }
            faces_vec.push((out, face_image));
        }
        let (buffers, faces_array): (Vec<Vec<u8>>, Vec<Image>) = faces_vec.into_iter().unzip();

        let mut faces_opt: Vec<Option<Image>> = (0..faces_array.len()).map(|_| None).collect();
        for (face_index, image) in faces_array.into_iter().enumerate() {
            if let (Some(new_face_idx), rotation) = map_face_and_image_rotation(Axis::Z, Rotate180, face_index)
            {
                rotate_image(&image, rotation);
                faces_opt[new_face_idx] = Some(image);
            }
        }

        let faces: Vec<Image> = faces_opt
            .into_iter()
            .map(|opt| opt.expect("expected every face to be assigned"))
            .collect();
        let _buffers = buffers; // keeps allocations alive until end of scope TODO: Remove this hack

        let mut scratch_image = ScratchImage::default();
        scratch_image.initialize_cube_from_images(faces.as_slice(), CP_FLAGS_NONE)?;
        Ok(scratch_image)
    }

    #[derive(Copy, Clone, Debug)]
    enum Axis { X, Y, Z }
    type Vec3 = (i8, i8, i8);


    impl Orientation {
        fn to_deg(self) -> u16 {
            match self {
                Rotate90 => 90,
                Rotate180 => 180,
                Rotate270 => 270,
            }
        }

        pub fn add(current: Option<Orientation>, step: Option<Orientation>) -> Option<Orientation> {
            match (current, step) {
                (None, None) => None,
                (Some(r), None) | (None, Some(r)) => Some(r),
                (Some(a), Some(b)) => {
                    let sum = (a.to_deg() + b.to_deg()) % 360;
                    match sum {
                        0 => None,
                        90 => Some(Rotate90),
                        180 => Some(Rotate180),
                        270 => Some(Rotate270),
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn rotate_vec(axis: Axis, rot: Orientation, (x,y,z): Vec3) -> Vec3 {
        match axis {
            Axis::X => match rot { //was y
                Rotate270  => ( z,  y, -x),
                Rotate180 => (-x,  y, -z),
                Rotate90 => (-z,  y,  x),
            },
            Axis::Y => match rot { //was z
                Rotate270  => (-y,  x,  z),
                Rotate180 => (-x, -y,  z),
                Rotate90 => ( y, -x,  z),
            },
            Axis::Z => match rot { //was x
                Rotate270  => ( x, -z,  y),
                Rotate180 => ( x, -y, -z),
                Rotate90 => ( x,  z, -y),
            },
        }
    }

    fn face_axes(face: usize) -> Option<(Vec3, Vec3, Vec3)> {
        match face {
            0 => Some((( 1,  0,  0),  (0,  0, -1),  (0, -1,  0))), // +X
            1 => Some(((-1,  0,  0),  (0,  0,  1),  (0, -1,  0))), // -X
            2 => Some((( 0,  1,  0),  (1,  0,  0),  (0,  0,  1))), // +Y
            3 => Some((( 0, -1,  0),  (1,  0,  0),  (0,  0, -1))), // -Y
            4 => Some((( 0,  0,  1),  (1,  0,  0),  (0, -1,  0))), // +Z
            5 => Some((( 0,  0, -1),  (-1, 0,  0),  (0, -1,  0))), // -Z
            _ => None,
        }
    }
    fn neg(v: Vec3) -> Vec3 { (-v.0, -v.1, -v.2) }

    fn map_face_and_image_rotation(axis: Axis, rot: Orientation, face_index: usize) -> (Option<usize>, Option<Orientation>) {
        let (n_src, r_src, _) = face_axes(face_index).unwrap();
        let n_rot = rotate_vec(axis, rot, n_src);
        let r_rot = rotate_vec(axis, rot, r_src);

        let dst = match n_rot {
            ( 1,  0,  0) => Some(0),
            (-1,  0,  0) => Some(1),
            ( 0,  1,  0) => Some(2),
            ( 0, -1,  0) => Some(3),
            ( 0,  0,  1) => Some(4),
            ( 0,  0, -1) => Some(5),
            _ => None,
        };

        let (_, r_dst, u_dst) = face_axes(dst.unwrap()).unwrap();

        let rot = match r_rot{
            v if v == r_dst => None,
            v if v == neg(r_dst) => Some(Rotate180),
            v if v == u_dst => Some(Rotate90),
            v if v == neg(u_dst) => Some(Rotate270),
            _ => unreachable!(),
        };

        (dst, rot)
    }

    fn map_face_and_image_rotations(
        face_index: usize,
        steps: Vec<(Axis, Orientation)>,
    ) -> (Option<usize>, Option<Orientation>) {
        let mut new_index = face_index;
        let mut new_rot: Option<Orientation> = None;
        for (axis, rot) in steps {
            let (new_face, step_face_rot) = map_face_and_image_rotation(axis, rot, new_index);
            let new_face = match new_face {
                Some(f) => f,
                None => return (None, None),
            };
            new_index = new_face;
            new_rot = Orientation::add(new_rot, step_face_rot);
        }
        (Some(new_index), new_rot)
    }
}