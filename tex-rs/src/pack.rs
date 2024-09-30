use crate::enums::{Dimensions, InterpretAs, RenderFormat, RenderResourceMiscFlags, TextureType};
use std::{fs, io, slice};
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, Write};
use std::path::Path;
use binrw::BinWrite;
use directxtex::{DXGI_FORMAT, TEX_COMPRESS_FLAGS, TEX_FILTER_FLAGS, TEX_THRESHOLD_DEFAULT, TGA_FLAGS};
use lz4::block::CompressionMode;
use thiserror::Error;
use crate::enums::RenderFormat::BC4;
use crate::texture_map::{AtlasData, MipblockData, TextureData, TextureMap, TextureMapHeaderV1, TextureMapHeaderV2, TextureMapHeaderV3, TextureMapInner};
use crate::pack::TexturePackerError::{DirectXTexError, PackingError};
use crate::WoaVersion;

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

/// TexturePacker struct that serves as both a builder and packer for TextureMap.
pub struct TexturePacker {
    texture_map: TextureMap,
}

impl TexturePacker {
    //Creates a new TexturePacker from a TextureMap
    pub fn new_from_texture_map(texture: TextureMap) -> Self {
        TexturePacker {
            texture_map: texture,
        }
    }

    /// Packs the TextureMap to a `Vec<u8>`.
    pub fn pack_to_vec(&self) -> Result<Vec<u8>, TexturePackerError> {
        let mut writer = Cursor::new(Vec::new());
        self.pack_internal(&mut writer)?;
        Ok(writer.into_inner())
    }

    /// Packs the TextureMap to a file at the specified path.
    pub fn pack_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), TexturePackerError> {
        let file = fs::File::create(path).map_err(TexturePackerError::IoError)?;
        let mut writer = BufWriter::new(file);
        self.pack_internal(&mut writer)?;
        Ok(())
    }

    /// Internal method to serialize the TextureMap.
    fn pack_internal<W: Write + Seek>(&self, writer: &mut W) -> Result<(), TexturePackerError> {
        self.texture_map
            .write_le_args(writer, ())
            .map_err(TexturePackerError::SerializationError)?;
        Ok(())
    }
}

/// Builder struct for constructing TextureMap instances.
pub struct TextureMapBuilder {
    woa_version: WoaVersion,
    texture_type: TextureType,
    texd_identifier: u32,
    flags: RenderResourceMiscFlags,
    width: u16,
    height: u16,
    format: RenderFormat,
    num_mip_levels: u8,
    default_mip_level: u8,
    interpret_as: InterpretAs,
    dimensions: Dimensions,
    mip_sizes: [u32; 14], // Adjust size based on actual mip levels
    compressed_mip_sizes: [u32; 14], // Adjust size based on actual mip levels
    atlas_data: Option<AtlasData>,
    data: Vec<u8>,
}

impl TextureMapBuilder {
    /// Creates a new TextureMapBuilder with default settings.
    pub fn new(woa_version: WoaVersion) -> Self {
        Self {
            woa_version,
            texture_type: TextureType::Colour,
            texd_identifier: 0xFFFFFF,
            flags: RenderResourceMiscFlags::default(),
            width: 256,
            height: 256,
            format: RenderFormat::BC7, // Default format
            num_mip_levels: 1,
            default_mip_level: 0,
            interpret_as: InterpretAs::Normal,
            dimensions: Dimensions::_2D,
            mip_sizes: [0; 14],
            compressed_mip_sizes: [0; 14],
            atlas_data: None,
            data: Vec::new(),
        }
    }

    // Builder methods for each field
    pub fn texture_type(mut self, texture_type: TextureType) -> Self {
        self.texture_type = texture_type;
        self
    }

    // pub fn texd_identifier(mut self, identifier: u32) -> Self {
    //     self.texd_identifier = identifier;
    //     self
    // }

    pub fn flags(mut self, flags: RenderResourceMiscFlags) -> Self {
        self.flags = flags;
        self
    }

    // pub fn default_mip_level(mut self, level: u8) -> Self {
    //     self.default_mip_level = level;
    //     self
    // }

    pub fn interpret_as(mut self, interpret_as: InterpretAs) -> Self {
        self.interpret_as = interpret_as;
        self
    }

    pub fn dimensions(mut self, dimensions: Dimensions) -> Self {
        self.dimensions = dimensions;
        self
    }

    pub fn atlas_data(mut self, atlas_data: AtlasData) -> Self {
        self.atlas_data = Some(atlas_data);
        self
    }

    pub fn load_tga<P: AsRef<Path>>(
        mut self,
        image_path: P,
        mip_levels: u8,
        compress: Option<RenderFormat>,
    ) -> Result<Self, TexturePackerError> {
        // Read the image data from the file
        let image_data = fs::read(&image_path).map_err(TexturePackerError::IoError)?;

        // Load the TGA image using DirectXTex
        let mut image = directxtex::ScratchImage::load_tga(
            image_data.as_slice(),
            TGA_FLAGS::TGA_FLAGS_NONE,
            None,
        ).map_err(TexturePackerError::DirectXTexError)?;

        // Extract image properties
        self.width = image.metadata().width as u16;
        self.height = image.metadata().height as u16;
        image = image.generate_mip_maps(TEX_FILTER_FLAGS::TEX_FILTER_LINEAR, mip_levels as usize)?;

        if !image.metadata().format.is_compressed() {
            if let Some(new_format) = compress {
                let format: RenderFormat = image.metadata().format.into();
                if format.num_channels() != new_format.num_channels() {
                    return Err(PackingError(format!("Invalid compression format provided, {:?} and {:?} don't share the same number of channels", format, new_format)));
                }
                let reqs = [DXGI_FORMAT::from(new_format).is_typeless(false), DXGI_FORMAT::from(new_format).is_planar(), DXGI_FORMAT::from(new_format).is_palettized()];
                if reqs.iter().any(|b| *b) {
                    return Err(PackingError(format!("Invalid compression format provided, the provided format is [typeless: {}, planar: {}, palettized: {}]", reqs[0], reqs[1], reqs[2])));
                }
                println!("Start compressing");
                image = image.compress(new_format.into(), TEX_COMPRESS_FLAGS::TEX_COMPRESS_BC7_QUICK, TEX_THRESHOLD_DEFAULT).map_err(DirectXTexError)?;
                println!("Done compressing");
            }
        }

        self.format = image.metadata().format.into();


        let generated_mip_levels = image.metadata().mip_levels.clamp(0, 14) as u8;
        self.num_mip_levels = generated_mip_levels;

        // Handle mip sizes
        for i in 0..generated_mip_levels as usize {
            let last: u32 = i
                .checked_sub(1)
                .and_then(|index| self.mip_sizes.get(index))
                .copied()
                .unwrap_or(0);
            self.mip_sizes[i] = last + image.image(i, 0, 0).map(|img| img.slice_pitch).unwrap_or(0) as u32;
        }


        self.data = Self::serialize_mipmaps(&image, generated_mip_levels)?;
        match self.woa_version {
            WoaVersion::HM3 => {
                let mut compressed_image_buffer = vec![];
                for mip in 0..generated_mip_levels as usize {
                    if let Some(mip_image) = image.image(mip, 0, 0) {
                        let mut cursor = Cursor::new(&self.data);
                        let mut mip_data = vec![0u8; mip_image.slice_pitch];
                        cursor.read(mip_data.as_mut_slice()).map_err(TexturePackerError::IoError)?;
                        let mip_compressed = lz4::block::compress(&*mip_data, Some(CompressionMode::HIGHCOMPRESSION(12)), false).map_err(|_| PackingError(format!("Failed to compress mip level {}", mip)))?;

                        let last: u32 = mip
                            .checked_sub(1)
                            .and_then(|index| self.compressed_mip_sizes.get(index))
                            .copied()
                            .unwrap_or(0);
                        self.compressed_mip_sizes[mip] = last + image.image(mip, 0, 0).map(|img| mip_compressed.len()).unwrap_or(0) as u32;

                        compressed_image_buffer.extend(mip_compressed);
                    }
                }
                self.data = compressed_image_buffer;
            }
            WoaVersion::HM2 => {
                self.compressed_mip_sizes = self.mip_sizes;
            }
            _ => {}
        }


        Ok(self)
    }

    /// Final build method to create a TextureMap.
    pub fn build(self) -> Result<TextureMap, TexturePackerError> {
        let mipblock = MipblockData{
            header: vec![],
            data: self.data,
        };
        let texture_map_inner = match self.woa_version {
            WoaVersion::HM2016 => {
                let header = TextureMapHeaderV1 {
                    type_: self.texture_type,
                    texd_identifier: self.texd_identifier,
                    flags: self.flags,
                    width: self.width,
                    height: self.height,
                    format: self.format,
                    num_mip_levels: self.num_mip_levels,
                    default_mip_level: self.default_mip_level,
                    interpret_as: self.interpret_as,
                    dimensions: Dimensions::_2D,
                    mip_sizes: self.mip_sizes,
                    has_atlas: self.atlas_data.is_some(),
                };
                TextureMapInner {
                    header,
                    atlas_data: self.atlas_data,
                    data: TextureData::Mipblock1(mipblock),
                }
                    .into()
            }
            WoaVersion::HM2 => {
                let header = TextureMapHeaderV2 {
                    type_: self.texture_type,
                    texd_identifier: self.texd_identifier,
                    flags: self.flags,
                    width: self.width,
                    height: self.height,
                    format: self.format,
                    num_mip_levels: self.num_mip_levels,
                    default_mip_level: self.default_mip_level,
                    mip_sizes: self.mip_sizes,
                    compressed_mip_sizes: self.compressed_mip_sizes,
                    has_atlas: self.atlas_data.is_some(),
                };
                TextureMapInner {
                    header,
                    atlas_data: self.atlas_data,
                    data: TextureData::Mipblock1(mipblock),
                }
                    .into()
            }
            WoaVersion::HM3 => {
                let header = TextureMapHeaderV3 {
                    type_: self.texture_type,
                    flags: self.flags,
                    width: self.width,
                    height: self.height,
                    format: self.format,
                    num_mip_levels: self.num_mip_levels,
                    default_mip_level: self.default_mip_level,
                    interpret_as: self.interpret_as,
                    dimensions: Dimensions::_2D,
                    mip_sizes: self.mip_sizes,
                    compressed_mip_sizes: self.compressed_mip_sizes,
                    has_atlas: self.atlas_data.is_some(),
                };
                TextureMapInner {
                    header,
                    atlas_data: self.atlas_data,
                    data: TextureData::Mipblock1(mipblock),
                }
                    .into()
            }
        };

        Ok(texture_map_inner)
    }

    fn serialize_mipmaps(
        image: &directxtex::ScratchImage,
        mip_levels: u8,
    ) -> Result<Vec<u8>, TexturePackerError> {
        let mut serialized = Vec::new();
        for mip in 0..mip_levels {
            if let Some(mip_image) = image.image(mip as usize, 0, 0) {
                unsafe { //ffs
                    if !mip_image.pixels.is_null() {
                        serialized.extend_from_slice(slice::from_raw_parts_mut(mip_image.pixels, mip_image.slice_pitch));
                    }
                }
            } else {
                return Err(PackingError(format!(
                    "Missing mip level {}",
                    mip
                )));
            }
        }
        Ok(serialized)
    }
}