use std::io::Read;
use std::ptr::NonNull;
use std::{slice};

use directxtex::{
    Image, ScratchImage, DDS_FLAGS_NONE, DXGI_FORMAT_R16G16B16A16_FLOAT,
    DXGI_FORMAT_R8G8B8A8_UNORM, TGA_FLAGS_NONE, TEX_FILTER_DEFAULT, TEX_THRESHOLD_DEFAULT,
};
use glacier_base::math::Vector3;
use glacier_texture::box_reflection::Orientation::*;
use glacier_texture::box_reflection::{BoxReflection, CubemapLayout, Orientation};
use rstest::rstest;

use crate::{read_fixture, write_fixture};

fn layout_short(layout: CubemapLayout) -> &'static str {
    match layout {
        CubemapLayout::HorizontalCross => "h-cross",
        CubemapLayout::VerticalCross => "v-cross",
        _ => unreachable!(),
    }
}

fn source_fixture(layout: CubemapLayout) -> Vec<u8> {
    read_fixture(format!("cubemap/{}-rot.tga", layout_short(layout)).as_str())
}

#[rstest]
#[ignore]
#[case([None, None, None], "identity")]
#[ignore]
#[case([None, None, Some(Rotate90)], "rot-z90")]
#[ignore]
#[case([None, None, Some(Rotate180)], "rot-z180")]
#[ignore]
#[case([None, None, Some(Rotate270)], "rot-z270")]
#[ignore]
#[case([None, Some(Rotate90), None], "rot-y90")]
#[ignore]
#[case([None, Some(Rotate90), Some(Rotate90)], "rot-y90z90")]
#[ignore]
#[case([None, Some(Rotate90), Some(Rotate180)], "rot-y90z180")]
#[ignore]
#[case([None, Some(Rotate90), Some(Rotate270)], "rot-y90z270")]
#[ignore]
#[case([None, Some(Rotate180), None], "rot-y180")]
#[ignore]
#[case([None, Some(Rotate180), Some(Rotate90)], "rot-y180z90")]
#[ignore]
#[case([None, Some(Rotate180), Some(Rotate180)], "rot-y180z180")]
#[ignore]
#[case([None, Some(Rotate180), Some(Rotate270)], "rot-y180z270")]
#[ignore]
#[case([None, Some(Rotate270), None], "rot-y270")]
#[ignore]
#[case([None, Some(Rotate270), Some(Rotate90)], "rot-y270z90")]
#[ignore]
#[case([None, Some(Rotate270), Some(Rotate180)], "rot-y270z180")]
#[ignore]
#[case([None, Some(Rotate270), Some(Rotate270)], "rot-y270z270")]
#[ignore]
#[case([Some(Rotate90), None, None], "rot-x90")]
#[ignore]
#[case([Some(Rotate90), None, Some(Rotate90)], "rot-x90z90")]
#[ignore]
#[case([Some(Rotate90), None, Some(Rotate180)], "rot-x90z180")]
#[ignore]
#[case([Some(Rotate90), None, Some(Rotate270)], "rot-x90z270")]
#[ignore]
#[case([Some(Rotate90), Some(Rotate180), None], "rot-x90y180")]
#[ignore]
#[case([Some(Rotate90), Some(Rotate180), Some(Rotate90)], "rot-x90y180z90")]
#[ignore]
#[case([Some(Rotate90), Some(Rotate180), Some(Rotate180)], "rot-x90y180z180")]
#[ignore]
#[case([Some(Rotate90), Some(Rotate180), Some(Rotate270)], "rot-x90y180z270")]
fn reauthor_rotation_fixture(
    #[case] rotation: [Option<Orientation>; 3],
    #[case] filename: &str,
    #[values(CubemapLayout::HorizontalCross, CubemapLayout::VerticalCross)]
    layout: CubemapLayout,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_tga = source_fixture(layout);
    let source_scratch = ScratchImage::load_tga(&source_tga, TGA_FLAGS_NONE, None)?;
    let source_dds = source_scratch.save_dds(DDS_FLAGS_NONE)?;

    let box_reflection =
        BoxReflection::from_dds(source_dds.buffer().to_vec(), Vector3::default())?;

    let out_dds = box_reflection.create_dds_with_rotation(Some(layout), rotation)?;
    let out_scratch = ScratchImage::load_dds(&out_dds, DDS_FLAGS_NONE, None, None)?
        .convert(
            DXGI_FORMAT_R16G16B16A16_FLOAT,
            TEX_FILTER_DEFAULT,
            TEX_THRESHOLD_DEFAULT,
        )?;

    let out = out_scratch.image(0, 0, 0).unwrap();

    let thumb = out.convert(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        TEX_FILTER_DEFAULT,
        TEX_THRESHOLD_DEFAULT,
    )?;
    let thumb = thumb.image(0, 0, 0).unwrap();

    let filename = format!("cubemap/rotated/{}-{}.tga", layout_short(layout), filename);
    write_fixture(&filename, thumb.save_tga(TGA_FLAGS_NONE, None)?.buffer());
    Ok(())
}

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
#[case([None, None, None], "identity")]
fn verify_rotating_layouts(
    #[case] rotation: [Option<Orientation>; 3],
    #[case] filename: &str,
    #[values(CubemapLayout::HorizontalCross, CubemapLayout::VerticalCross)]
    layout: CubemapLayout,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout_short = match layout {
        CubemapLayout::HorizontalCross => "h-cross",
        CubemapLayout::VerticalCross => "v-cross",
        _ => unreachable!(),
    };

    let source_tga = read_fixture(format!("cubemap/{}-rot.tga", layout_short).as_str());
    let expected_tga =
        read_fixture(format!("cubemap/rotated/{}-{}.tga", layout_short, filename).as_str());

    let source_scratch = ScratchImage::load_tga(&source_tga, TGA_FLAGS_NONE, None)?;
    let source_dds = source_scratch.save_dds(DDS_FLAGS_NONE)?;
    let box_reflection =
        BoxReflection::from_dds(source_dds.buffer().to_vec(), Vector3::default())?;

    let out_dds = box_reflection.create_dds_with_rotation(Some(layout), rotation)?;
    let out_scratch = ScratchImage::load_dds(&out_dds, DDS_FLAGS_NONE, None, None)?
        .convert(
            DXGI_FORMAT_R16G16B16A16_FLOAT,
            TEX_FILTER_DEFAULT,
            TEX_THRESHOLD_DEFAULT,
        )?;

    let expected_scratch = ScratchImage::load_tga(&expected_tga, TGA_FLAGS_NONE, None)?
        .convert(
            DXGI_FORMAT_R16G16B16A16_FLOAT,
            TEX_FILTER_DEFAULT,
            TEX_THRESHOLD_DEFAULT,
        )?;

    let out_layout_img = out_scratch.image(0, 0, 0).unwrap();
    let expected_dds = expected_scratch.image(0, 0, 0).unwrap();

    assert_eq!(out_layout_img.width, expected_dds.width);
    assert_eq!(out_layout_img.height, expected_dds.height);
    assert_eq!(
        checksum(image_pixels(out_layout_img).unwrap().as_slice()),
        checksum(image_pixels(expected_dds).unwrap().as_slice())
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
    let raw_buffer = raw_slice.to_vec();
    Some(raw_buffer)
}