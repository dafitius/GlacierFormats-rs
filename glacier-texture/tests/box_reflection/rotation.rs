use std::{slice};
use std::io::Read;
use std::ptr::NonNull;
use glacier_texture::box_reflection::{BoxReflection, CubemapLayout};
use glacier_texture::box_reflection::cubemap_utils::{compose_layout, compose_layout_with_rotation, decompose_layout, Orientation};
use directxtex::{Image, ScratchImage, DXGI_FORMAT_R16G16B16A16_FLOAT, TGA_FLAGS_NONE, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT, DXGI_FORMAT_R8G8B8A8_UNORM};
use rstest::rstest;
use glacier_texture::box_reflection::cubemap_utils::Orientation::*;
use crate::read_fixture;

#[rstest]
#[case([None, None, Some(Rotate90)], "rot-z90")]
#[case([None, None, Some(Rotate180)], "rot-z180")]
#[case([None, None, Some(Rotate270)], "rot-z270")]
#[case([None, Some(Rotate90), None], "rot-y90")]
#[case([None, Some(Rotate90), Some(Rotate90)], "rot-y90z90")]
#[case([None, Some(Rotate90), Some(Rotate180)], "rot-y90z180")]
#[case([None, Some(Rotate90), Some(Rotate270)], "rot-y90z270")]
#[case([None, Some(Rotate180), None], "rot-y180")]
#[case([None, Some(Rotate180), Some(Rotate90)], "rot-y180z90")]
#[case([None, Some(Rotate180), Some(Rotate180)], "rot-y180z180")]
#[case([None, Some(Rotate180), Some(Rotate270)], "rot-y180z270")]
#[case([None, Some(Rotate270), None], "rot-y270")]
#[case([None, Some(Rotate270), Some(Rotate90)], "rot-y270z90")]
#[case([None, Some(Rotate270), Some(Rotate180)], "rot-y270z180")]
#[case([None, Some(Rotate270), Some(Rotate270)], "rot-y270z270")]
#[case([Some(Rotate90), None, None], "rot-x90")]
#[case([Some(Rotate90), None, Some(Rotate90)], "rot-x90z90")]
#[case([Some(Rotate90), None, Some(Rotate180)], "rot-x90z180")]
#[case([Some(Rotate90), None, Some(Rotate270)], "rot-x90z270")]
#[case([Some(Rotate90), Some(Rotate180), None], "rot-x90y180")]
#[case([Some(Rotate90), Some(Rotate180), Some(Rotate90)], "rot-x90y180z90")]
#[case([Some(Rotate90), Some(Rotate180), Some(Rotate180)], "rot-x90y180z180")]
#[case([Some(Rotate90), Some(Rotate180), Some(Rotate270)], "rot-x90y180z270")]
fn verify_rotating_layouts(
    #[case] rotation: [Option<Orientation>; 3],
    #[case] filename: &str,
    #[values(
        CubemapLayout::HorizontalCross, CubemapLayout::VerticalCross
    )]
    layout: CubemapLayout,
) -> Result<(), Box<dyn std::error::Error>> {

    let layout_short = match layout {
        CubemapLayout::HorizontalCross => "h-cross",
        CubemapLayout::VerticalCross => "v-cross",
        _ => unreachable!(),
    };

    let mut source_dds = ScratchImage::load_tga(&read_fixture(format!("cubemap/{}-rot.tga", layout_short).as_str()), TGA_FLAGS_NONE, None)?;
    source_dds = source_dds.convert(DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT)?;

    let mut expected_dds = ScratchImage::load_tga(&read_fixture(format!("cubemap/rotated/{}-{}.tga", layout_short, filename).as_str()), TGA_FLAGS_NONE, None)?;
    expected_dds = expected_dds.convert(DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT)?;

    let expected_dds = expected_dds.image(0,0,0).unwrap();

    let source_img = source_dds.image(0, 0, 0).unwrap();
    let cubemap = decompose_layout(source_img, layout)?;
    let out_layout_img = compose_layout_with_rotation(&cubemap, layout, rotation)?;
    let out_layout_img = out_layout_img.image(0,0,0).unwrap();

    // == Debug ==

    let thumb = out_layout_img.convert(DXGI_FORMAT_R8G8B8A8_UNORM, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT)?;
    let thumb = thumb.image(0,0,0).unwrap();
    std::fs::write(format!("/home/dafitius/Documents/GitHub/GlacierFormats-rs/target/tmp/{}-{}.tga", layout_short, filename), &thumb.save_tga(TGA_FLAGS_NONE, None).unwrap().buffer())?;

    // ============

    assert_eq!(out_layout_img.width, expected_dds.width);
    assert_eq!(out_layout_img.height, expected_dds.height);
    assert_eq!(checksum(image_pixels(&out_layout_img).unwrap().as_slice()), checksum(image_pixels(expected_dds).unwrap().as_slice()));
    Ok(())
}

pub fn checksum(msg: &[u8]) -> u16 {
    let mut crc: u16 = 0x0;
    for byte in msg.iter() {
        let mut x = ((crc >> 8) ^ (*byte as u16)) & 255;
        x ^= x >> 4;
        crc = (crc << 8) ^ (x << 12) ^ (x << 5) ^ x;
    }
    crc
}

pub(crate) fn image_pixels(image: &Image) -> Option<Vec<u8>> {
    let pixels = NonNull::new(image.pixels)?;
    let scanlines = image.format.compute_scanlines(image.height);
    let buffer_size = image.row_pitch.checked_mul(scanlines)?;
    let raw_slice = unsafe { slice::from_raw_parts(pixels.as_ptr(), buffer_size) };
    let raw_buffer = raw_slice.to_vec();
    Some(raw_buffer)
}