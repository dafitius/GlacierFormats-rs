use std::{fs, io};
use std::path::Path;
use directxtex::{Blob, CP_FLAGS, DDS_FLAGS, DXGI_FORMAT, HResultError, Image, ScratchImage, TexMetadata, TGA_FLAGS};
use thiserror::Error;
use crate::convert::TextureConversionError::DirectXTexError;
use crate::texture_map::{MipLevel, RenderFormat, TextureMap};
use crate::texture_map::RenderFormat::R16G16B16A16;

#[derive(Error, Debug)]
pub enum TextureConversionError{
    #[error("Io error {0}")]
    IoError(#[from] io::Error),

    #[error("DirectxTex error {0}")]
    DirectXTexError(#[from] HResultError),

    #[error("Invalid texture: {0}")]
    InvalidTexture(String),

    #[error("Tried to read mip level {0}, which is out of bounds [0..{}]")]
    MipOutOfBounds(usize, usize),
}

pub fn create_dds(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let header = tex.get_header();

    let mut mips = (0..tex.get_num_mip_levels()).filter_map(|i| -> Option<MipLevel> {
        if let Ok(mip) = tex.get_mip_level(i) {if mip.height > 0 && mip.width > 0 { Some(mip) } else {None} } else {None}
    }).collect::<Vec<_>>();

    let first_mip = mips.first().ok_or(TextureConversionError::InvalidTexture("There are no mips on the texture".to_string()))?;

    let meta_data = TexMetadata {
        width: first_mip.width,
        height: first_mip.height,
        depth: 0,
        array_size: 1,
        mip_levels: mips.len(),
        misc_flags: 0,
        misc_flags2: 0,
        format: header.format.into(),
        dimension: header.dimensions.into(),
    };

    let images_result: Result<Vec<Image>, TextureConversionError> = mips.iter_mut().map(|mip| -> Result<Image, TextureConversionError> {
        let pitch = DXGI_FORMAT::from(header.format).compute_pitch(mip.width, mip.height, CP_FLAGS::CP_FLAGS_NONE).map_err(|e| DirectXTexError(e))?;

        Ok(Image {
            width: mip.width,
            height: mip.height,
            format: header.format.into(),
            row_pitch: pitch.row,
            slice_pitch: pitch.slice,
            pixels: mip.data.as_mut_ptr(),
        })
    }).collect();

    let images = images_result?;

    let blob = directxtex::save_dds(images.as_slice(), &meta_data, DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT).map_err(|e| DirectXTexError(e))?;
    Ok(Vec::from(blob.buffer()))
}

pub fn create_tga(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let dds = create_dds(tex)?;
    let scratch_image = ScratchImage::load_dds(dds.as_slice(), DDS_FLAGS::DDS_FLAGS_NONE, None, None).map_err(|e| DirectXTexError(e))?;

    //TODO: convert the 2-channel textures to the correct color space (R8G8, BC5)

    if tex.get_header().format == R16G16B16A16{
        todo!("Convert the image to R8G8B8A8 here");
    }

    let format = match tex.get_header().format{

        RenderFormat::DXT1 | RenderFormat::DXT3 |
        RenderFormat::DXT5 | RenderFormat::BC5 |
        RenderFormat::BC7 => {
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM
        }
        RenderFormat::BC4 => {
            DXGI_FORMAT::DXGI_FORMAT_A8_UNORM
        }
        _ => { DXGI_FORMAT::DXGI_FORMAT_UNKNOWN }
    };

    let decompressed = directxtex::decompress(scratch_image.images(), scratch_image.metadata(), format).unwrap();
    let blob = decompressed.image(0, 0, 0).unwrap().save_tga(TGA_FLAGS::TGA_FLAGS_NONE, None).map_err(|e|DirectXTexError(e))?;
    Ok(Vec::from(blob.buffer()))
}


pub fn create_mip_dds(tex: &TextureMap, mip_level: usize) -> Result<Vec<u8>, TextureConversionError> {
    let header = tex.get_header();
    if let Ok(mut mip) = tex.get_mip_level(mip_level) {
        let meta_data = TexMetadata {
            width: mip.width,
            height: mip.height,
            depth: 0,
            array_size: 1,
            mip_levels: 1,
            misc_flags: 0,
            misc_flags2: 0,
            format: header.format.into(),
            dimension: header.dimensions.into(),
        };
        let pitch = DXGI_FORMAT::from(header.format).compute_pitch(mip.width, mip.height, CP_FLAGS::CP_FLAGS_NONE).map_err(|e| DirectXTexError(e))?;

        let image = Image {
            width: mip.width,
            height: mip.height,
            format: header.format.into(),
            row_pitch: pitch.row,
            slice_pitch: pitch.slice,
            pixels: mip.data.as_mut_ptr(),
        };

        let blob = directxtex::save_dds(&[image], &meta_data, DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT).map_err(|e| DirectXTexError(e))?;
        Ok(Vec::from(blob.buffer()))
    }else{
        Err(TextureConversionError::MipOutOfBounds(mip_level, tex.get_num_mip_levels()))
    }
}