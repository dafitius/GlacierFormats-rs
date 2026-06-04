use std::{slice};
use std::ptr::NonNull;
use glacier_texture::box_reflection::{BoxReflection, CubemapLayout};
use directxtex::{Image, ScratchImage, DXGI_FORMAT_R16G16B16A16_FLOAT, TGA_FLAGS_NONE, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT, DDS_FLAGS_NONE};
use rstest::rstest;
use glacier_base::math::Vector3;
use glacier_texture::box_reflection::Orientation::*;
use crate::read_fixture;

#[test]
fn verify_from_tile_counts() {
    assert_eq!(CubemapLayout::from_tile_counts(1, 6), Some(CubemapLayout::VerticalStrip));
    assert_eq!(CubemapLayout::from_tile_counts(6, 1), Some(CubemapLayout::HorizontalStrip));
    assert_eq!(CubemapLayout::from_tile_counts(4, 3), Some(CubemapLayout::HorizontalCross));
    assert_eq!(CubemapLayout::from_tile_counts(3, 4), Some(CubemapLayout::VerticalCross));
    assert_eq!(CubemapLayout::from_tile_counts(2, 2), None);
}

fn layout_fixture(layout: CubemapLayout) -> Vec<u8> {
    match layout {
        CubemapLayout::HorizontalStrip => read_fixture("cubemap/h-line-abs.tga"),
        CubemapLayout::VerticalStrip => read_fixture("cubemap/v-line-abs.tga"),
        CubemapLayout::HorizontalCross => read_fixture("cubemap/h-cross-abs.tga"),
        CubemapLayout::VerticalCross => read_fixture("cubemap/v-cross-abs.tga"),
    }
}

#[rstest]
fn verify_converting_layouts(
    #[values(
        CubemapLayout::HorizontalStrip,
        CubemapLayout::VerticalStrip,
        CubemapLayout::HorizontalCross,
        CubemapLayout::VerticalCross
    )]
    layout: CubemapLayout,
    #[values(
        CubemapLayout::HorizontalStrip,
        CubemapLayout::VerticalStrip,
        CubemapLayout::HorizontalCross,
        CubemapLayout::VerticalCross
    )]
    expected_layout: CubemapLayout,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_tga = layout_fixture(layout);
    let expected_tga = layout_fixture(expected_layout);

    // Load source fixture and build a BoxReflection through the public API.
    let source_img = ScratchImage::load_tga(&source_tga, TGA_FLAGS_NONE, None)?;
    let source_dds = source_img.save_dds(DDS_FLAGS_NONE)?;
    let box_reflection = BoxReflection::from_dds(source_dds.buffer().to_vec(), Vector3::default())?;

    // Export to the requested layout through the public API.
    let out_dds = box_reflection.create_dds_with_rotation(
        Some(expected_layout),
        [None, None, Some(Rotate270)], // undo standard -90 z rotation
    )?;
    let out_scratch = ScratchImage::load_dds(&out_dds, DDS_FLAGS_NONE, None, None)?
        .convert(
            DXGI_FORMAT_R16G16B16A16_FLOAT,
            TEX_FILTER_DEFAULT,
            TEX_THRESHOLD_DEFAULT,
        )?;

    // Load expected fixture for comparison.
    let expected_scratch = ScratchImage::load_tga(&expected_tga, TGA_FLAGS_NONE, None)?
        .convert(
            DXGI_FORMAT_R16G16B16A16_FLOAT,
            TEX_FILTER_DEFAULT,
            TEX_THRESHOLD_DEFAULT,
        )?;

    let out_img = out_scratch.image(0, 0, 0).unwrap();
    let expected_img = expected_scratch.image(0, 0, 0).unwrap();

    assert_eq!(out_img.width, expected_img.width);
    assert_eq!(out_img.height, expected_img.height);
    assert_eq!(
        checksum(image_pixels(out_img).unwrap().as_slice()),
        checksum(image_pixels(expected_img).unwrap().as_slice())
    );

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
    Some(raw_slice.to_vec())
}