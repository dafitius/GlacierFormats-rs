use crate::convert::TextureConversionError::DirectXTexError;
use crate::enums::RenderFormat;
use crate::texture_map::{MipLevel, TextureMap};
use directxtex::{
    HResultError, Image, ScratchImage, TexMetadata, CP_FLAGS, DDS_FLAGS, DXGI_FORMAT,
    TEX_FILTER_FLAGS, TEX_THRESHOLD_DEFAULT, TGA_FLAGS,
};
use png::ColorType;
use std::io;
use std::io::{BufWriter, Cursor, Write};
use thiserror::Error;

#[cfg(feature = "image")]
use crate::image::TextureMapDecoder;
#[cfg(feature = "image")]
use image::{DynamicImage, ImageResult};


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

/// Converts a `TextureMap` into a DDS (DirectDraw Surface) image file.
pub fn create_dds(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let mut mips = (0..tex.num_mip_levels())
        .filter_map(|i| -> Option<MipLevel> {
            if let Ok(mip) = tex.mipmap(i) {
                if mip.height > 0 && mip.width > 0 {
                    Some(mip)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let first_mip = mips.first().ok_or(TextureConversionError::InvalidTexture(
        "There are no textures in the data".to_string(),
    ))?;

    let meta_data = TexMetadata {
        width: first_mip.width,
        height: first_mip.height,
        depth: 0,
        array_size: 1,
        mip_levels: mips.len(),
        misc_flags: 0,
        misc_flags2: 0,
        format: tex.format().into(),
        dimension: tex.dimensions().into(),
    };

    let images_result: Result<Vec<Image>, TextureConversionError> = mips
        .iter_mut()
        .map(|mip| -> Result<Image, TextureConversionError> {
            let pitch = DXGI_FORMAT::from(tex.format())
                .compute_pitch(mip.width, mip.height, CP_FLAGS::CP_FLAGS_NONE)
                .map_err(DirectXTexError)?;

            Ok(Image {
                width: mip.width,
                height: mip.height,
                format: tex.format().into(),
                row_pitch: pitch.row,
                slice_pitch: pitch.slice,
                pixels: mip.data.as_mut_ptr(),
            })
        })
        .collect();

    let images = images_result?;

    let blob = directxtex::save_dds(
        images.as_slice(),
        &meta_data,
        DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT,
    )
    .map_err(DirectXTexError)?;
    Ok(Vec::from(blob.buffer()))
}


/// Converts a `TextureMap` into a TGA (Targa) image file.
/// # Warning
/// The TGA format does **not** support 16-bit per channel formats such as `R16G16B16A16`.
/// If the input texture uses this format, the function may fail or produce incorrect output.
pub fn create_tga(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let dds = create_dds(tex)?;
    let mut scratch_image = ScratchImage::load_dds(
        dds.as_slice(),
        DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT,
        None,
        None,
    )
    .map_err(DirectXTexError)?;
    scratch_image = decompress_dds(tex, scratch_image)?;
    let blob = scratch_image
        .image(0, 0, 0)
        .unwrap()
        .save_tga(TGA_FLAGS::TGA_FLAGS_NONE, None)
        .map_err(DirectXTexError)?;
    Ok(Vec::from(blob.buffer()))
}

/// Converts a `TextureMap` into a PNG image file.
pub fn create_png(tex: &TextureMap) -> Result<Vec<u8>, TextureConversionError> {
    let dds = create_dds(tex)?;
    let mut scratch_image = ScratchImage::load_dds(
        dds.as_slice(),
        DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT,
        None,
        None,
    ).map_err(DirectXTexError)?;

    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut w = BufWriter::new(cursor);

    scratch_image = decompress_dds(tex, scratch_image)?;

    let render_format: RenderFormat = scratch_image.metadata().format.try_into().unwrap();

    let color_type = match render_format {
        RenderFormat::A8 => Some(ColorType::Grayscale),
        RenderFormat::R16G16B16A16 => Some(ColorType::Rgba),
        RenderFormat::R8G8B8A8 => Some(ColorType::Rgb),
        RenderFormat::R8G8 => Some(ColorType::Grayscale),
        _ => None,
    };

    let bit_depth = match render_format {
        RenderFormat::R16G16B16A16 => png::BitDepth::Sixteen,
        _ => png::BitDepth::Eight,
    };

    let mut encoder = png::Encoder::new(
        &mut w,
        scratch_image.metadata().width as u32,
        scratch_image.metadata().height as u32,
    );
    encoder.set_color(color_type.unwrap());
    encoder.set_depth(bit_depth);
    let mut writer = encoder.write_header().unwrap();

    let blob = scratch_image
        .image(0, 0, 0)
        .unwrap()
        .save_dds(DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT)?;

    writer.write_image_data(blob.buffer()).unwrap(); // Save

    writer.finish().unwrap();
    w.flush()?;

    let cursor = w.into_inner().unwrap();
    Ok(cursor.into_inner())
}

#[cfg(feature = "image")]
pub fn create_dynamic_image(tex: &TextureMap) -> ImageResult<DynamicImage> {
    DynamicImage::from_decoder(TextureMapDecoder::from_texture_map(tex.clone()))
}

pub(crate) fn decompress_dds(
    tex: &TextureMap,
    scratch_image: ScratchImage,
) -> Result<ScratchImage, TextureConversionError> {
    let mut scratch_image = scratch_image;
    if tex.format().is_compressed() {
        scratch_image = directxtex::decompress(
            scratch_image.images(),
            scratch_image.metadata(),
            match tex.format().num_channels() {
                1 => DXGI_FORMAT::DXGI_FORMAT_A8_UNORM,
                2 => DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM,
                4 => DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
                _ => DXGI_FORMAT::DXGI_FORMAT_UNKNOWN,
            },
        )
        .map_err(DirectXTexError)?
    }

    if tex.format() == RenderFormat::R16G16B16A16 {
        scratch_image = directxtex::convert(
            scratch_image.images(),
            scratch_image.metadata(),
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
            TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT | TEX_FILTER_FLAGS::TEX_FILTER_FORCE_NON_WIC,
            TEX_THRESHOLD_DEFAULT,
        )
        .map_err(DirectXTexError)?;
    }

    //generate missing blue channel
    if tex.format().num_channels() == 2 {
        scratch_image = directxtex::convert(
            scratch_image.images(),
            scratch_image.metadata(),
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
            TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT,
            TEX_THRESHOLD_DEFAULT,
        )
        .map_err(DirectXTexError)?;

        for pixel in scratch_image.pixels_mut().chunks_mut(4) {
            if pixel.len() != 4 {
                continue;
            }
            let x = pixel[0] as f64 / 255.0;
            let y = pixel[1] as f64 / 255.0;
            pixel[2] = (f64::sqrt(1.0 - (x * x - y * y)) * 255.0) as u8;
        }
    }
    Ok(scratch_image)
}

pub fn create_mip_dds(
    tex: &TextureMap,
    mip_level: usize,
    decompress: bool,
) -> Result<Vec<u8>, TextureConversionError> {
    if let Ok(mut mip) = tex.mipmap(mip_level) {
        let meta_data = TexMetadata {
            width: mip.width,
            height: mip.height,
            depth: 0,
            array_size: 1,
            mip_levels: 1,
            misc_flags: 0,
            misc_flags2: 0,
            format: tex.format().into(),
            dimension: tex.dimensions().into(),
        };
        let pitch = DXGI_FORMAT::from(tex.format())
            .compute_pitch(mip.width, mip.height, CP_FLAGS::CP_FLAGS_NONE)
            .map_err(DirectXTexError)?;

        let image = Image {
            width: mip.width,
            height: mip.height,
            format: tex.format().into(),
            row_pitch: pitch.row,
            slice_pitch: pitch.slice,
            pixels: mip.data.as_mut_ptr(),
        };

        let mut blob =
            directxtex::save_dds(&[image], &meta_data, DDS_FLAGS::DDS_FLAGS_FORCE_DX10_EXT)
                .map_err(DirectXTexError)?;
        if decompress {
            let dds = ScratchImage::load_dds(blob.buffer(), DDS_FLAGS::DDS_FLAGS_NONE, None, None)
                .map_err(DirectXTexError)?;
            let new_dds = decompress_dds(tex, dds)?;
            blob = directxtex::save_dds(
                new_dds.images(),
                new_dds.metadata(),
                DDS_FLAGS::DDS_FLAGS_NONE,
            )
            .map_err(DirectXTexError)?;
        }
        Ok(Vec::from(blob.buffer()))
    } else {
        Err(TextureConversionError::MipOutOfBounds(
            mip_level,
            tex.num_mip_levels(),
        ))
    }
}
