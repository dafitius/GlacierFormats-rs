use crate::read_fixture;
use binrw::BinRead;
use directxtex::{DDS_FLAGS, DXGI_FORMAT, TEX_FILTER_DEFAULT, TGA_FLAGS};
use glacier_texture::enums::{InterpretAs, RenderFormat, TextureType};
use glacier_texture::mipblock::MipblockData;
use glacier_texture::pack::{MipFilter, MipLevels, TextureMapBuilder};
use glacier_texture::texture_map::TextureMap;
use glacier_texture::GlacierGame;
use image::ImageFormat;
use rstest::rstest;
use std::io::Cursor;

fn load_dds(bytes: &[u8]) -> Result<directxtex::ScratchImage, Box<dyn std::error::Error>> {
    directxtex::ScratchImage::load_dds(bytes, DDS_FLAGS::DDS_FLAGS_NONE, None, None)
        .map_err(Into::into)
}

fn decompress_if_needed(
    image: directxtex::ScratchImage,
) -> Result<directxtex::ScratchImage, Box<dyn std::error::Error>> {
    if !image.metadata().format.is_compressed() {
        return Ok(image);
    }

    image
        .decompress(match image.metadata().format {
            DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM
            | DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM
            | DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM
            | DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM => DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM => DXGI_FORMAT::DXGI_FORMAT_A8_UNORM,
            DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM => DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM,
            _ => DXGI_FORMAT::DXGI_FORMAT_UNKNOWN,
        })
        .map_err(Into::into)
}

fn convert_to_rgba8_if_needed(
    image: directxtex::ScratchImage,
) -> Result<directxtex::ScratchImage, Box<dyn std::error::Error>> {
    if image.metadata().format == DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM {
        return Ok(image);
    }

    image
        .convert(
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
            TEX_FILTER_DEFAULT,
            0.5,
        )
        .map_err(Into::into)
}

fn to_tga(
    image: &directxtex::ScratchImage,
) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    let tga_buffer = image
        .image(0, 0, 0)
        .unwrap()
        .save_tga(TGA_FLAGS::TGA_FLAGS_NONE, None)?;

    image::load_from_memory_with_format(tga_buffer.buffer(), ImageFormat::Tga).map_err(Into::into)
}

#[rstest]
#[case("source/R16G16B16A16_FLOAT.dds", RenderFormat::R16G16B16A16)] //1
#[case("source/R8G8B8A8_UNORM.dds", RenderFormat::R8G8B8A8)] //2
#[case("source/R8G8_UNORM.dds", RenderFormat::R8G8)] //3
#[case("source/A8_UNORM.dds", RenderFormat::A8)] //4
#[case("source/BC1_UNORM.dds", RenderFormat::BC1)] //5
#[case("source/BC2_UNORM.dds", RenderFormat::BC2)] //6
#[case("source/BC3_UNORM.dds", RenderFormat::BC3)] //7
#[case("source/BC4_UNORM.dds", RenderFormat::BC4)] //8
#[case("source/BC5_UNORM.dds", RenderFormat::BC5)] //9
#[case("source/BC7_UNORM.dds", RenderFormat::BC7)] //10
pub fn dds_packing_text_texd(
    #[case] source_path: &str,
    #[case] source_format: RenderFormat,
    #[values(GlacierGame::HM2016, GlacierGame::HM2, GlacierGame::HM3)] game_version: GlacierGame,
    #[values(MipLevels::All, MipLevels::Limit(2))] mip_mode: MipLevels,
    #[values(true, false)] texd_mode: bool,
    #[values(true, false)] read_texd: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let dds = read_fixture(source_path);
    let dds_old = dds.clone();

    let texture = TextureMapBuilder::from_dds(Cursor::new(dds))?
        .interpret_as(InterpretAs::Normal)
        .texture_type(TextureType::Colour)
        .with_mip_filter(MipFilter::Linear)
        .with_num_mip_levels(mip_mode)
        .with_mipblock1(texd_mode)
        .with_format(source_format)
        .build(game_version)?;

    let text = texture.pack_to_vec()?;
    let mut texture_map = TextureMap::from_memory(&text, game_version)?;

    if read_texd {
        if let Some(mipblock) = texture.mipblock1() {
            let texd = mipblock.pack_to_vec(game_version)?;
            let block = MipblockData::from_memory(&texd, game_version)?;
            texture_map.set_mipblock1(block);
        }
    }

    let rebuilt_dds = glacier_texture::convert::create_dds(&mut texture_map)?;

    let dynamic_dds = load_dds(&dds_old)?;
    let dynamic_rebuilt_dds = load_dds(&rebuilt_dds)?;

    assert_eq!(
        dynamic_dds.metadata().format,
        dynamic_rebuilt_dds.metadata().format
    );

    let incomplete = texd_mode && !read_texd;
    if !incomplete {
        assert_eq!(
            dynamic_dds.metadata().width,
            dynamic_rebuilt_dds.metadata().width
        );
        assert_eq!(
            dynamic_dds.metadata().height,
            dynamic_rebuilt_dds.metadata().height
        );
    }

    let dynamic_dds = convert_to_rgba8_if_needed(decompress_if_needed(dynamic_dds)?)?;
    let dynamic_rebuilt_dds =
        convert_to_rgba8_if_needed(decompress_if_needed(dynamic_rebuilt_dds)?)?;

    let dynamic_tga = to_tga(&dynamic_dds)?;
    let dynamic_rebuilt_tga = to_tga(&dynamic_rebuilt_dds)?;

    if !incomplete {
        assert_eq!(dynamic_tga.as_rgb8(), dynamic_rebuilt_tga.as_rgb8());
        assert_eq!(
            dynamic_tga.as_luma_alpha8(),
            dynamic_rebuilt_tga.as_luma_alpha8()
        );
    }
    Ok(())
}
