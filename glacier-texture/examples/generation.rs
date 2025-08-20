use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use rpkg_rs::resource::package_builder::{PackageBuilder, PackageResourceBuilder};
use rpkg_rs::resource::pdefs::PartitionId;
use rpkg_rs::resource::resource_package::{PackageVersion, ReferenceType, ResourceReferenceFlags, ResourceReferenceFlagsStandard};
use rpkg_rs::resource::resource_partition::PatchId;
use rpkg_rs::resource::runtime_resource_id::RuntimeResourceID;
use glacier_texture::enums::{InterpretAs, RenderFormat, TextureType};
use glacier_texture::pack::MipFilter::Linear;
use glacier_texture::pack::TextureMapBuilder;
use glacier_texture::WoaVersion;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    //Parameters
    let tga_path = PathBuf::from("./target/texture.tga");
    let text_rrid = RuntimeResourceID::from_hex_string("000210D1CF04E4E4")?;
    let texd_rrid = RuntimeResourceID::from_hex_string("00752CEA9F76AB7E")?;
    let woa_version = WoaVersion::HM3;

    let partition_id: PartitionId = "chunk12".parse().unwrap();
    let patch_id: PatchId = PatchId::Patch(5);

    let add_texd = true;

    //create a package
    let mut package = PackageBuilder::new_with_patch_id(partition_id, patch_id);

    //Create texture from tga
    let tga_data = Cursor::new(fs::read(tga_path)?);
    let texture =
        TextureMapBuilder::from_tga(tga_data)?
            .interpret_as(InterpretAs::Normal)
            .texture_type(TextureType::Colour)
            .with_mip_filter(Linear)
            .with_mipblock1(add_texd)
            .with_format(RenderFormat::BC1).build(woa_version)?;

    //Add resources to package
    let mut texture_resource = PackageResourceBuilder::from_memory(
        text_rrid,
        "TEXT",
        texture.pack_to_vec()?,
        match woa_version {
            WoaVersion::HM2016 => { Some(12) }
            WoaVersion::HM2 => { None }
            WoaVersion::HM3 => { None }
        },
        true,
    )?;
    texture_resource.with_memory_requirements(0xFFFFFFFF, texture.video_memory_requirement() as u32);

    if texture.has_mipblock1() {
        let mipblock1 = texture.mipblock1().unwrap();
        let highmip_resource = PackageResourceBuilder::from_memory(
            texd_rrid,
            "TEXD",
            mipblock1.pack_to_vec(woa_version)?,
            None,
            false)?;
        texture_resource.with_memory_requirements(0xFFFFFFFF, mipblock1.video_memory_requirement() as u32);

        texture_resource.with_reference(texd_rrid, ResourceReferenceFlags::Standard(ResourceReferenceFlagsStandard::new().with_reference_type(ReferenceType::WEAK)));
        package.with_resource(highmip_resource);
    }
    package.with_resource(texture_resource);

    package.build_to_file(PackageVersion::RPKGv2, "./target".to_owned())?;
    Ok(())
}
