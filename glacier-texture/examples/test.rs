use glacier_texture::texture_map::TextureMap;
use glacier_texture::GlacierGame;
use image::ImageFormat;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    //Parameters
    // let r8_path = PathBuf::from("D:\\David\\Hitman-modding\\temp\\0018941C98370007.TEXT");
    let r16_path = PathBuf::from("/media/dafitius/980 PRO/HitmanProjects/tmp/000B1BC3C75B15D9.TEXT");
    let woa_version = GlacierGame::HM3;


    //Create texture from tga
    // let r8_texture = TextureMap::from_file(r8_path, woa_version)?;
    // let r8_tga = tex_rs::convert::create_tga(&r8_texture)?;
    // fs::write("./target/r8.tga", r8_tga)?;

    let r16_texture = TextureMap::from_file(r16_path, woa_version)?;
    let r8_tga = glacier_texture::convert::create_tga(&r16_texture)?;
    let r16_dds = glacier_texture::convert::create_dds(&r16_texture)?;
    let r16_hdr = glacier_texture::convert::create_dynamic_image(&r16_texture)?;
    r16_hdr.save_with_format("./target/r16.exr", ImageFormat::OpenExr)?;
    fs::write("./target/r8.tga", r8_tga)?;
    fs::write("./target/r16.dds", r16_dds)?;
    Ok(())
}
