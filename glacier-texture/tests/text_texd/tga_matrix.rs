use std::io::Cursor;
use binrw::BinRead;
use directxtex::convert;
use glacier_texture::enums::{InterpretAs, RenderFormat, TextureType};
use glacier_texture::mipblock::MipblockData;
use glacier_texture::pack::{MipFilter, MipLevels, TextureMapBuilder};
use glacier_texture::texture_map::TextureMap;
use glacier_texture::GlacierGame;
use rstest::rstest;
use crate::read_fixture;

#[rstest]
#[case("source/A8_UNORM.tga", RenderFormat::A8)]
#[case("source/R8G8_UNORM.tga", RenderFormat::R8G8)]
#[case("source/R8G8B8_UNORM.tga", RenderFormat::R8G8B8A8)]
#[case("source/R8G8B8A8_UNORM.tga", RenderFormat::R8G8B8A8)]
fn tga_packing_text_texd(
    #[case] source_path: &str, #[case] source_format: RenderFormat,
    #[values(GlacierGame::HM2016, GlacierGame::HM2, GlacierGame::HM3)] game_version: GlacierGame,
    #[values(MipLevels::All, MipLevels::Limit(2))] mip_mode: MipLevels,
    #[values(true, false)] texd_mode: bool,
    #[values(true, false)] read_texd: bool) -> Result<(), Box<dyn std::error::Error>> {

    let tga = read_fixture(source_path);

    let texture = TextureMapBuilder::from_tga(Cursor::new(tga))?
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
        if let Some(mipblock) = texture.mipblock1(){
            let texd = mipblock.pack_to_vec(game_version)?;
            let block = MipblockData::from_memory(&texd, game_version)?;
            texture_map.set_mipblock1(block);
        }
    }

    let rebuilt_tga = glacier_texture::convert::create_tga(&mut texture_map)?;

    let dynamic_tga = image::load_from_memory_with_format(&rebuilt_tga, image::ImageFormat::Tga)?;
    let dynamic_rebuilt_tga = image::load_from_memory_with_format(&rebuilt_tga, image::ImageFormat::Tga)?;

    assert_eq!(dynamic_tga.has_alpha(), dynamic_rebuilt_tga.has_alpha());

    let incomplete = texd_mode && !read_texd;
    if !incomplete {
        assert_eq!(dynamic_tga.width(), dynamic_rebuilt_tga.width());
        assert_eq!(dynamic_tga.height(), dynamic_rebuilt_tga.height());
        assert_eq!(dynamic_tga.as_rgb8(), dynamic_rebuilt_tga.as_rgb8());
        assert_eq!(dynamic_tga.as_luma_alpha8(), dynamic_rebuilt_tga.as_luma_alpha8());
    }
    Ok(())
}