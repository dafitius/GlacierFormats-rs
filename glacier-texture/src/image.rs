use crate::atlas::AtlasData;
use crate::convert::create_dds;
use crate::convert::TextureConversionError::DirectXTexError;
use crate::enums::RenderFormat;
use crate::mipblock::MipblockData;
use crate::pack::{TextureMapBuilder, TextureMapParameters, TexturePackerError};
use crate::texture_map::{TextureMap};
use crate::WoaVersion;
use binrw::BinRead;
use directxtex::{HResultError, ScratchImage, CP_FLAGS, DDS_FLAGS, DXGI_FORMAT, TEX_FILTER_FLAGS};
use image::error::{EncodingError, ImageFormatHint};
use image::{ColorType, ExtendedColorType, ImageDecoder, ImageEncoder, ImageError, ImageResult};
use std::io::{BufRead, Seek, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TextureMapEncodeError {
    #[error("DXGI conversion failed for color type {0:?}")]
    DxgiConversion(ExtendedColorType),
    #[error("Failed DirectXTex operation {0}")]
    DirectXTexError(#[from] HResultError),
    #[error("Failed to pack texture")]
    Packer(#[from] TexturePackerError),
    #[error("IO error {0}")]
    IOError(#[from] std::io::Error),
}

impl From<TextureMapEncodeError> for ImageError {
    fn from(e: TextureMapEncodeError) -> Self {
        ImageError::Encoding(EncodingError::new(
            ImageFormatHint::Name("TextureMap".to_owned()),
            e.to_string(),
        ))
    }
}

pub struct TextureMapEncoder<TW: Write, DW: Write> {
    text_writer: TW,
    texd_writer: Option<DW>,
    woa_version: WoaVersion,
    texture_parameters: Option<TextureMapParameters>,
    atlas_data: Option<AtlasData>,
}

impl<TW: Write, DW: Write> TextureMapEncoder<TW, DW> {
    pub fn new(
        text_writer: TW,
        texd_writer: Option<DW>,
        woa_version: WoaVersion,
        texture_parameters: Option<TextureMapParameters>,
        atlas_data: Option<AtlasData>,
    ) -> TextureMapEncoder<TW, DW> {
        TextureMapEncoder {
            text_writer,
            texd_writer,
            woa_version,
            texture_parameters,
            atlas_data,
        }
    }
}

impl<TW: Write, DW: Write> ImageEncoder for TextureMapEncoder<TW, DW> {
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let scratch_image = dynamic_image_to_scratch_image(buf, width, height, color_type)?;
        let mut builder = TextureMapBuilder::from_scratch_image(scratch_image)
            .map_err(TextureMapEncodeError::Packer)?;

        if let Some(params) = self.texture_parameters {
            builder = builder.with_params(params);
        }

        if let Some(atlas_data) = self.atlas_data {
            builder = builder.with_atlas(atlas_data);
        }

        let text = builder
            .build(self.woa_version)
            .map_err(TextureMapEncodeError::Packer)?;
        let text_data = text.pack_to_vec().map_err(TextureMapEncodeError::Packer)?;

        let mut text_writer = self.text_writer;
        text_writer.write_all(&text_data)?;

        if let Some(mut texd_writer) = self.texd_writer {
            if let Some(texd) = text.mipblock1() {
                let texd_data = texd
                    .pack_to_vec(self.woa_version)
                    .map_err(TextureMapEncodeError::Packer)?;
                texd_writer.write_all(&texd_data)?;
            }
        }
        Ok(())
    }
}

pub fn dynamic_image_to_scratch_image(buf: &[u8], width: u32, height: u32, color_type: ExtendedColorType) -> Result<ScratchImage, TextureMapEncodeError> {
    let dxgi_format = helpers::color_type_to_dxgi(color_type)
        .ok_or(TextureMapEncodeError::DxgiConversion(color_type))?;
    let slice_pitch = dxgi_format
        .compute_pitch(width as usize, height as usize, CP_FLAGS::CP_FLAGS_NONE)
        .map_err(TextureMapEncodeError::DirectXTexError)?;

    let width = width as usize;
    let height = height as usize;

    let maybe_converted;
    let pixels = match color_type {
        ExtendedColorType::Rgb8 | ExtendedColorType::Bgr8 => {
            maybe_converted = Some(helpers::rgb8_to_rgba8(buf));
            maybe_converted.as_ref().unwrap().as_ptr() as *mut u8
        }
        ExtendedColorType::Rgb16 => {
            maybe_converted = Some(helpers::rgb16_to_rgba16(buf));
            maybe_converted.as_ref().unwrap().as_ptr() as *mut u8
        }
        _ => buf.as_ptr() as *mut u8,
    };

    let image = directxtex::Image {
        width,
        height,
        format: dxgi_format,
        row_pitch: slice_pitch.row,
        slice_pitch: slice_pitch.slice,
        pixels,
    };

    image
        .resize(width, height, TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT)
        .map_err(TextureMapEncodeError::DirectXTexError)
}


pub struct TextureMapDecoder {
    texture: TextureMap,
}

impl TextureMapDecoder {
    pub fn new<TR: BufRead + Seek, DR: BufRead + Seek>(
        mut text_reader: TR,
        texd_reader: Option<DR>,
        woa_version: WoaVersion,
    ) -> Self {
        let mut texture = TextureMap::read_le_args(&mut text_reader, (woa_version,)).unwrap();
        if let Some(mut texd_reader) = texd_reader {
            let mut buf = Vec::new();
            texd_reader.read_to_end(&mut buf).unwrap();
            let mip_data = MipblockData::from_memory(&buf, woa_version).unwrap();
            texture.set_mipblock1(mip_data);
        }
        Self { texture }
    }

    pub fn from_texture_map(texture: TextureMap) -> Self {
        Self { texture }
    }
}

impl ImageDecoder for TextureMapDecoder {
    fn dimensions(&self) -> (u32, u32) {
        (self.texture.width() as u32, self.texture.height() as u32)
    }

    fn color_type(&self) -> ColorType {
        match self.texture.format() {
            RenderFormat::R16G16B16A16 => ColorType::Rgba16,
            RenderFormat::R8G8B8A8 => ColorType::Rgba8,
            RenderFormat::R8G8 => ColorType::La8,
            RenderFormat::A8 => ColorType::L8,
            RenderFormat::BC1 => ColorType::Rgba8,
            RenderFormat::BC2 => ColorType::Rgba8,
            RenderFormat::BC3 => ColorType::Rgba8,
            RenderFormat::BC4 => ColorType::L8,
            RenderFormat::BC5 => ColorType::La8,
            RenderFormat::BC7 => ColorType::Rgba8,
        }
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
    where
        Self: Sized,
    {
        let dds = create_dds(&self.texture).unwrap();
        let mut scratch_image = ScratchImage::load_dds(
            dds.as_slice(),
            DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT,
            None,
            None,
        )
        .map_err(DirectXTexError)
        .unwrap();

        scratch_image = crate::convert::decompress_dds(&self.texture, scratch_image).unwrap();

        let blob = scratch_image
            .image(0, 0, 0)
            .unwrap()
            .save_dds(DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT)
            .unwrap();

        let data = blob.buffer();
        buf.copy_from_slice(&data[data.len() - buf.len()..]);

        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

mod helpers {
    use super::*;
    pub(super) fn color_type_to_dxgi(color_type: ExtendedColorType) -> Option<DXGI_FORMAT> {
        match color_type {
            ExtendedColorType::A8 => Some(DXGI_FORMAT::DXGI_FORMAT_A8_UNORM),
            ExtendedColorType::L8 => Some(DXGI_FORMAT::DXGI_FORMAT_R8_UNORM),
            ExtendedColorType::La8 => Some(DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM),
            ExtendedColorType::Rgb8 => Some(DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM), // Needs additional alpha channel
            ExtendedColorType::Rgba8 => Some(DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM),
            ExtendedColorType::L16 => Some(DXGI_FORMAT::DXGI_FORMAT_R16_UNORM),
            ExtendedColorType::La16 => Some(DXGI_FORMAT::DXGI_FORMAT_R16G16_UNORM),
            ExtendedColorType::Rgb16 => Some(DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_UNORM), // Needs additional alpha channel
            ExtendedColorType::Rgba16 => Some(DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_UNORM),
            ExtendedColorType::Bgr8 => Some(DXGI_FORMAT::DXGI_FORMAT_B8G8R8X8_UNORM), // Needs additional alpha channel
            ExtendedColorType::Bgra8 => Some(DXGI_FORMAT::DXGI_FORMAT_B8G8R8A8_UNORM),
            ExtendedColorType::Rgb32F => Some(DXGI_FORMAT::DXGI_FORMAT_R32G32B32_FLOAT),
            ExtendedColorType::Rgba32F => Some(DXGI_FORMAT::DXGI_FORMAT_R32G32B32A32_FLOAT),
            _ => None,
        }
    }

    pub(super) fn rgb8_to_rgba8(rgb: &[u8]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
        for chunk in rgb.chunks(3) {
            rgba.push(chunk[0]);
            rgba.push(chunk[1]);
            rgba.push(chunk[2]);
            rgba.push(0xFF);
        }
        rgba
    }

    pub(super) fn rgb16_to_rgba16(rgb: &[u8]) -> Vec<u8> {
        assert_eq!(rgb.len() % 6, 0, "Input length must be divisible by 6.");
        let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
        for chunk in rgb.chunks(6) {
            rgba.extend_from_slice(&chunk[0..2]);
            rgba.extend_from_slice(&chunk[2..4]);
            rgba.extend_from_slice(&chunk[4..6]);
            rgba.extend_from_slice(&0xFFFFu16.to_le_bytes());
        }
        rgba
    }
}
