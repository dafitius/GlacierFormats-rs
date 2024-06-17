use std::io;
use directxtex::{CP_FLAGS, DDS_FLAGS, DXGI_FORMAT, HResultError, Image, ScratchImage, TEX_FILTER_FLAGS, TEX_THRESHOLD_DEFAULT, TexMetadata, TGA_FLAGS};
use thiserror::Error;
use crate::convert::TextureConversionError::DirectXTexError;
use crate::texture_map::{MipLevel, RenderFormat, TextureMap};

#[derive(Error, Debug)]
pub enum TextureConversionError {
    #[error("Io error {0}")]
    IoError(#[from] io::Error),

    #[error("DirectxTex error {0}")]
    DirectXTexError(#[from] HResultError),

    #[error("Invalid texture: {0}")]
    InvalidTexture(String),

    #[error("Tried to read mip level {0}, which is out of bounds [0..{0}]")]
    MipOutOfBounds(usize, usize),
}

pub fn create_dds(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let header = tex.get_header();

    let mut mips = (0..tex.get_num_mip_levels()).filter_map(|i| -> Option<MipLevel> {
        if let Ok(mip) = tex.get_mip_level(i) { if mip.height > 0 && mip.width > 0 { Some(mip) } else { None } } else { None }
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
        let pitch = DXGI_FORMAT::from(header.format).compute_pitch(mip.width, mip.height, CP_FLAGS::CP_FLAGS_NONE).map_err(DirectXTexError)?;

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

    let blob = directxtex::save_dds(images.as_slice(), &meta_data, DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT).map_err(DirectXTexError)?;
    Ok(Vec::from(blob.buffer()))
}
pub fn create_tga(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let dds = create_dds(tex)?;
    let mut scratch_image = ScratchImage::load_dds(dds.as_slice(), DDS_FLAGS::DDS_FLAGS_NONE, None, None).map_err(DirectXTexError)?;

    if tex.get_header().format.is_compressed() {
        scratch_image = directxtex::decompress(scratch_image.images(), scratch_image.metadata(), match tex.get_header().format.num_channels() {
            1 => DXGI_FORMAT::DXGI_FORMAT_A8_UNORM,
            2 => DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM,
            4 => DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
            _ => DXGI_FORMAT::DXGI_FORMAT_UNKNOWN,
        }).map_err(DirectXTexError)?
    }

    if tex.get_header().format == RenderFormat::R16G16B16A16 {
        scratch_image = directxtex::convert(scratch_image.images(), scratch_image.metadata(), DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM, TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT).map_err(DirectXTexError)?;
    }

    //generate missing blue channel
    if tex.get_header().format.num_channels() == 2 {
        scratch_image = directxtex::convert(scratch_image.images(), scratch_image.metadata(), DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM, TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT).map_err(DirectXTexError)?;

        for pixel in scratch_image.pixels_mut().chunks_mut(4) {
            if pixel.len() != 4 {
                continue;
            }
            let x = pixel[0] as f64 / 255.0;
            let y = pixel[1] as f64 / 255.0;
            pixel[2] = (f64::sqrt(1.0 - (x * x - y * y)) * 255.0) as u8;
        };
    }

    let blob = scratch_image.image(0, 0, 0).unwrap().save_tga(TGA_FLAGS::TGA_FLAGS_NONE, None).map_err(DirectXTexError)?;
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
        let pitch = DXGI_FORMAT::from(header.format).compute_pitch(mip.width, mip.height, CP_FLAGS::CP_FLAGS_NONE).map_err(DirectXTexError)?;

        let image = Image {
            width: mip.width,
            height: mip.height,
            format: header.format.into(),
            row_pitch: pitch.row,
            slice_pitch: pitch.slice,
            pixels: mip.data.as_mut_ptr(),
        };

        let blob = directxtex::save_dds(&[image], &meta_data, DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT).map_err(DirectXTexError)?;
        Ok(Vec::from(blob.buffer()))
    } else {
        Err(TextureConversionError::MipOutOfBounds(mip_level, tex.get_num_mip_levels()))
    }
}