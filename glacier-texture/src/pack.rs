use crate::atlas::AtlasData;
use crate::enums::*;
use crate::mipblock::MipblockData;
use crate::pack::TexturePackerError::{DirectXTexError, PackingError};
use crate::texture_map::{
    TextureData, TextureMap, TextureMapHeaderV1, TextureMapHeaderV2, TextureMapHeaderV3,
    TextureMapInner,
};
use crate::{convert, WoaVersion};
use directxtex::{
    Image, ScratchImage, DXGI_FORMAT, TEX_COMPRESS_FLAGS, TEX_FILTER_FLAGS, TEX_THRESHOLD_DEFAULT,
    TGA_FLAGS,
};
use lz4::block::CompressionMode;
use std::cmp::max;
use std::io::{Cursor, Read};
use std::ptr::NonNull;
use std::{io, slice};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TexturePackerError {
    #[error("Error serializing the texture: {0}")]
    SerializationError(#[from] binrw::Error),

    #[error("Failed to read data: {0}")]
    IoError(#[from] io::Error),

    #[error("DirectX error: {0}")]
    DirectXTexError(#[from] directxtex::HResultError),

    #[error("Error building texture: {0}")]
    PackingError(String),
}

pub enum MipLevels {
    All,
    Limit(u8),
}

pub enum MipFilter {
    Nearest,
    Linear,
    Cubic,
    Box,
}

struct TextureMapParams {
    texture_type: TextureType,
    interpret_as: InterpretAs,
    dimensions: Dimensions,
    flags: TextureFlagsInner,
    format: RenderFormat,
    num_mip_levels: MipLevels,
    default_mip_level: u8,
    texd_identifier: u32,
    mip_filter: MipFilter,
}

impl TextureMapParams {
    pub fn new(format: RenderFormat) -> Self {
        Self {
            texture_type: TextureType::Colour,
            interpret_as: InterpretAs::Normal,
            dimensions: Dimensions::_2D,

            flags: TextureFlagsInner::default().with_unknown3(true),
            format,
            num_mip_levels: MipLevels::All,
            default_mip_level: 0,
            texd_identifier: 0x4000,
            mip_filter: MipFilter::Box,
        }
    }
}

/// Builder struct for constructing TextureMap instances.
/// Will enable the [`unknown3`] flag by default.
pub struct TextureMapBuilder {
    params: TextureMapParams,
    atlas_data: Option<AtlasData>,
    image: ScratchImage,
    use_mipblock1: bool,
}

impl TextureMapBuilder {
    /// Creates a new TextureMapBuilder with default settings.
    pub fn from_tga<R: Read>(mut reader: R) -> Result<Self, TexturePackerError> {
        let mut image_data = vec![];
        reader
            .read_to_end(&mut image_data)
            .map_err(TexturePackerError::IoError)?;
        let image = ScratchImage::load_tga(image_data.as_slice(), TGA_FLAGS::TGA_FLAGS_NONE, None)
            .map_err(DirectXTexError)?;

        let metadata = image.metadata();
        let render_format = metadata.format.try_into().or_else(|_err| {
            let bits_per_pixel = metadata.format.bits_per_pixel();
            let bits_per_color = metadata.format.bits_per_color();
            let num_channels = bits_per_pixel.checked_div(bits_per_color).unwrap_or(0);

            match (bits_per_pixel, bits_per_color) {
                (8, 8) => Ok(RenderFormat::A8),
                (16, 8) => Ok(RenderFormat::R8G8),
                (24, 8) => Ok(RenderFormat::R8G8B8A8),
                (32, 8) => Ok(RenderFormat::R8G8B8A8),
                (64, 16) => Ok(RenderFormat::R16G16B16A16),
                _ => Err(PackingError(format!(
                    "Unsupported render format: bpp={}, bpc={}, channels={:?}, format={:?}",
                    bits_per_pixel, bits_per_color, num_channels, metadata.format
                ))),
            }
        })?;

        Ok(Self {
            params: TextureMapParams::new(render_format),
            atlas_data: None,
            image,
            use_mipblock1: true,
        })
    }

    pub fn from_texture_map(texture: &TextureMap) -> Result<Self, TexturePackerError> {
        let mut builder = convert::create_tga(texture)
            .map(|tga| {
                let reader = Cursor::new(tga);
                Self::from_tga(reader)
            })
            .map_err(|e| PackingError(format!("Failed to convert texture: {e}")))??;

        builder.atlas_data = texture.atlas().clone();
        builder.params.texture_type = texture.texture_type();
        if let Some(interpret_as) = texture.interpret_as() {
            builder.params.interpret_as = interpret_as;
        }
        Ok(builder)
    }

    // Builder methods for each field
    pub fn texture_type(mut self, texture_type: TextureType) -> Self {
        self.params.texture_type = texture_type;
        self
    }

    pub fn with_default_mip_level(mut self, level: u8) -> Self {
        self.params.default_mip_level = level;
        self
    }

    pub fn with_num_mip_levels(mut self, levels: MipLevels) -> Self {
        self.params.num_mip_levels = levels;
        self
    }

    pub fn with_format(mut self, format: RenderFormat) -> Self {
        self.params.format = format;
        self
    }

    pub fn interpret_as(mut self, interpret_as: InterpretAs) -> Self {
        self.params.interpret_as = interpret_as;
        self
    }

    pub fn with_mip_filter(mut self, mip_filter: MipFilter) -> Self {
        self.params.mip_filter = mip_filter;
        self
    }

    pub fn with_atlas(mut self, atlas_data: AtlasData) -> Self {
        self.atlas_data = Some(atlas_data);
        self.params.flags = self.params.flags.with_atlas(true);
        self
    }

    pub fn with_mipblock1(mut self, enabled: bool) -> Self {
        self.use_mipblock1 = enabled;
        self
    }

    pub fn with_flags(mut self, flags: TextureFlags) -> Self {
        self.params.flags = flags.inner;
        self
    }

    #[cfg(feature = "unstable")]
    pub fn with_dimensions(mut self, dimensions: Dimensions) -> Self {
        self.params.dimensions = dimensions;
        self
    }

    #[cfg(feature = "unstable")]
    pub fn with_texd_id(mut self, texd_id: u32) -> Self {
        self.params.texd_identifier = texd_id;
        self
    }

    ///Convert the image to a different format.
    /// It is assumed that the input image is not compressed
    fn convert_to_format(
        image: ScratchImage,
        new_format: DXGI_FORMAT,
    ) -> Result<ScratchImage, TexturePackerError> {
        let reqs = [
            new_format.is_typeless(false),
            new_format.is_planar(),
            new_format.is_palettized(),
        ];
        if reqs.iter().any(|b| *b) {
            return Err(PackingError(format!("Invalid compression format provided, the provided format is [typeless: {}, planar: {}, palettized: {}]", reqs[0], reqs[1], reqs[2])));
        }

        Ok(match new_format.is_compressed() {
            true => image
                .compress(
                    new_format,
                    TEX_COMPRESS_FLAGS::TEX_COMPRESS_BC7_QUICK,
                    TEX_THRESHOLD_DEFAULT,
                )
                .map_err(DirectXTexError)?,
            false => image
                .convert(
                    new_format,
                    TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT,
                    TEX_THRESHOLD_DEFAULT,
                )
                .map_err(DirectXTexError)?,
        })
    }

    /// Final build method to create a TextureMap.
    pub fn build(self, woa_version: WoaVersion) -> Result<TextureMap, TexturePackerError> {
        let width = self.image.metadata().width as u16;
        let height = self.image.metadata().height as u16;

        if !width.is_power_of_two() {
            return Err(PackingError(format!(
                "Width ({width}) is not a power of two!"
            )));
        }

        if !height.is_power_of_two() {
            return Err(PackingError(format!(
                "Height ({height}) is not a power of two!"
            )));
        }

        let mut image = self.image.generate_mip_maps(
            match self.params.mip_filter {
                MipFilter::Nearest => TEX_FILTER_FLAGS::TEX_FILTER_POINT,
                MipFilter::Linear => TEX_FILTER_FLAGS::TEX_FILTER_LINEAR,
                MipFilter::Cubic => TEX_FILTER_FLAGS::TEX_FILTER_CUBIC,
                MipFilter::Box => TEX_FILTER_FLAGS::TEX_FILTER_BOX,
            } | TEX_FILTER_FLAGS::TEX_FILTER_FORCE_NON_WIC,
            match self.params.num_mip_levels {
                MipLevels::All => 0,
                MipLevels::Limit(n) => n as usize,
            },
        )?;

        let target_format = self.params.format.into();
        if self.image.metadata().format != target_format {
            image = Self::convert_to_format(image, target_format)?;
        }

        let generated_mip_levels = image.metadata().mip_levels.clamp(0, 14) as u8;
        let num_mip_levels = generated_mip_levels;

        // Handle mip sizes
        let mut mip_sizes = [0u32; 14];
        for i in 0..generated_mip_levels as usize {
            let last: u32 = i
                .checked_sub(1)
                .and_then(|index| mip_sizes.get(index))
                .copied()
                .unwrap_or(0);
            mip_sizes[i] =
                last + image.image(i, 0, 0).map(|img| img.slice_pitch).unwrap_or(0) as u32;
        }

        let mut data = Self::serialize_mipmaps(&image, generated_mip_levels)?;
        let mut compressed_mip_sizes = mip_sizes;
        if woa_version == WoaVersion::HM3 {
            let mut compressed_image_buffer = vec![];
            let mut cursor = Cursor::new(&data);
            for mip in 0..generated_mip_levels as usize {
                if let Some(mip_image) = image.image(mip, 0, 0) {
                    let mut mip_data = vec![0u8; mip_image.slice_pitch];
                    cursor
                        .read(mip_data.as_mut_slice())
                        .map_err(TexturePackerError::IoError)?;
                    let mip_compressed = lz4::block::compress(
                        &mip_data,
                        Some(CompressionMode::HIGHCOMPRESSION(12)),
                        false,
                    )
                    .map_err(|_| PackingError(format!("Failed to compress mip level {mip}")))?;

                    let last: u32 = mip
                        .checked_sub(1)
                        .and_then(|index| compressed_mip_sizes.get(index))
                        .copied()
                        .unwrap_or(0);
                    compressed_mip_sizes[mip] = last
                        + image
                            .image(mip, 0, 0)
                            .map(|_| mip_compressed.len())
                            .unwrap_or(0) as u32;

                    compressed_image_buffer.extend(mip_compressed);
                }
            }
            data = compressed_image_buffer;
        }

        let texture_data = if self.use_mipblock1 {
            TextureData::Mipblock1(MipblockData {
                video_memory_requirement: (mip_sizes.first().copied().unwrap_or(0x0)
                    + mip_sizes.get(1).copied().unwrap_or(0x0))
                    as usize,
                header: vec![],
                data,
            })
        } else {
            TextureData::Tex(data)
        };

        let texture_map_inner = match woa_version {
            WoaVersion::HM2016 => {
                let header = TextureMapHeaderV1 {
                    type_: self.params.texture_type,
                    texd_identifier: self.params.texd_identifier,
                    #[cfg(feature = "unstable")]
                    flags: self.params.flags,
                    #[cfg(not(feature = "unstable"))]
                    flags: TextureFlagsInner::default(), //detached from builder
                    width,
                    height,
                    format: self.params.format,
                    num_mip_levels,
                    default_mip_level: self.params.default_mip_level,
                    interpret_as: self.params.interpret_as,
                    dimensions: self.params.dimensions,
                    mip_sizes,
                    has_atlas: self.atlas_data.is_some(),
                };
                TextureMapInner {
                    header,
                    atlas_data: self.atlas_data,
                    data: texture_data,
                }
                .into()
            }
            WoaVersion::HM2 => {
                let header = TextureMapHeaderV2 {
                    type_: self.params.texture_type,
                    texd_identifier: self.params.texd_identifier,
                    #[cfg(feature = "unstable")]
                    flags: self.params.flags,
                    #[cfg(not(feature = "unstable"))]
                    flags: TextureFlagsInner::default(), //detached from builder
                    width,
                    height,
                    format: self.params.format,
                    num_mip_levels,
                    default_mip_level: max(self.params.default_mip_level, 1), //H2 crashes with index 0
                    mip_sizes,
                    compressed_mip_sizes,
                    has_atlas: self.atlas_data.is_some(),
                };
                TextureMapInner {
                    header,
                    atlas_data: self.atlas_data,
                    data: texture_data,
                }
                .into()
            }
            WoaVersion::HM3 => {
                let header = TextureMapHeaderV3 {
                    type_: self.params.texture_type,
                    flags: self.params.flags,
                    width,
                    height,
                    format: self.params.format,
                    num_mip_levels,
                    default_mip_level: self.params.default_mip_level,
                    interpret_as: self.params.interpret_as,
                    dimensions: self.params.dimensions,
                    mip_sizes,
                    compressed_mip_sizes,
                    has_atlas: self.atlas_data.is_some(),
                };
                TextureMapInner {
                    header,
                    atlas_data: self.atlas_data,
                    data: texture_data,
                }
                .into()
            }
        };

        Ok(texture_map_inner)
    }

    fn process_mip_image(mip_image: &Image) -> Option<Vec<u8>> {
        let pixels = NonNull::new(mip_image.pixels)?;
        let scanlines = mip_image.format.compute_scanlines(mip_image.height);
        let buffer_size = mip_image.row_pitch.checked_mul(scanlines)?;
        let raw_slice = unsafe { slice::from_raw_parts(pixels.as_ptr(), buffer_size) };
        let raw_buffer = raw_slice.to_vec();
        Some(raw_buffer)
    }

    fn serialize_mipmaps(
        image: &directxtex::ScratchImage,
        mip_levels: u8,
    ) -> Result<Vec<u8>, TexturePackerError> {
        let mut serialized = Vec::new();
        for mip in 0..mip_levels {
            if let Some(mip_image) = image.image(mip as usize, 0, 0) {
                let buffer = Self::process_mip_image(mip_image).unwrap_or(vec![]);
                serialized.extend_from_slice(buffer.as_slice());
            } else {
                return Err(PackingError(format!("Missing mip level {mip}")));
            }
        }
        Ok(serialized)
    }
}
