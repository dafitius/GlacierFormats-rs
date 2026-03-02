use std::{slice};
use std::ptr::NonNull;
use glacier_texture::box_reflection::{BoxReflection, CubemapLayout};
use glacier_texture::box_reflection::cubemap_utils::{compose_layout, decompose_layout};
use directxtex::{Image, ScratchImage, DXGI_FORMAT_R16G16B16A16_FLOAT, TGA_FLAGS_NONE, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT};
use rstest::rstest;
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
        CubemapLayout::HorizontalStrip, CubemapLayout::VerticalStrip,
        CubemapLayout::HorizontalCross, CubemapLayout::VerticalCross
    )]
    layout: CubemapLayout,
    #[values(
        CubemapLayout::HorizontalStrip, CubemapLayout::VerticalStrip,
        CubemapLayout::HorizontalCross, CubemapLayout::VerticalCross
    )]
    expected_layout: CubemapLayout,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut source_dds = ScratchImage::load_tga(&layout_fixture(layout), TGA_FLAGS_NONE, None)?;
    source_dds = source_dds.convert(DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT)?;
    let mut expected_dds = ScratchImage::load_tga(&layout_fixture(expected_layout), TGA_FLAGS_NONE, None)?;
    expected_dds = expected_dds.convert(DXGI_FORMAT_R16G16B16A16_FLOAT, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT)?;

    let expected_dds = expected_dds.image(0,0,0).unwrap();

    let source_img = source_dds.image(0, 0, 0).unwrap();
    // Original -> Cubemap -> other layout
    let cubemap = decompose_layout(source_img, layout)?;
    let out_layout_img = compose_layout(&cubemap, expected_layout, false)?;

    let out_layout_img = out_layout_img.image(0,0,0).unwrap();

    assert_eq!(out_layout_img.width, expected_dds.width);
    assert_eq!(out_layout_img.height, expected_dds.height);
    assert_eq!(image_pixels(&out_layout_img), image_pixels(expected_dds));
    Ok(())
}

pub(crate) fn image_pixels(image: &Image) -> Option<Vec<u8>> {
    let pixels = NonNull::new(image.pixels)?;
    let scanlines = image.format.compute_scanlines(image.height);
    let buffer_size = image.row_pitch.checked_mul(scanlines)?;
    let raw_slice = unsafe { slice::from_raw_parts(pixels.as_ptr(), buffer_size) };
    let raw_buffer = raw_slice.to_vec();
    Some(raw_buffer)
}