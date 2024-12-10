use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use rpkg_rs::resource::package_builder::{PackageBuilder, PackageResourceBuilder};
use rpkg_rs::resource::pdefs::PartitionId;
use rpkg_rs::resource::resource_package::{PackageVersion, ReferenceType, ResourceReferenceFlags, ResourceReferenceFlagsStandard};
use rpkg_rs::resource::resource_partition::PatchId;
use rpkg_rs::resource::runtime_resource_id::RuntimeResourceID;
use glacier_texture::texture_map::TextureMap;
use glacier_texture::WoaVersion;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    //Parameters
    let r8_path = PathBuf::from("D:\\David\\Hitman-modding\\temp\\0018941C98370007.TEXT");
    let r16_path = PathBuf::from("D:\\David\\Hitman-modding\\temp\\000B1BC3C75B15D9.TEXT");
    let woa_version = WoaVersion::HM3;


    //Create texture from tga
    // let r8_texture = TextureMap::from_file(r8_path, woa_version)?;
    // let r8_tga = tex_rs::convert::create_tga(&r8_texture)?;
    // fs::write("./target/r8.tga", r8_tga)?;

    let r16_texture = TextureMap::from_file(r16_path, woa_version)?;
    let r16_tga = glacier_texture::convert::create_tga(&r16_texture)?;
    fs::write("./target/r16.tga", r16_tga)?;

    Ok(())
}
