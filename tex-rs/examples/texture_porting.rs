use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use binrw::BinRead;
use rpkg_rs::resource::package_builder::{PackageBuilder, PackageResourceBuilder};
use rpkg_rs::resource::pdefs::{PartitionId, PartitionType};
use rpkg_rs::resource::resource_package::{PackageVersion, ReferenceType, ResourceReferenceFlags, ResourceReferenceFlagsStandard};
use rpkg_rs::resource::resource_partition::PatchId;
use rpkg_rs::resource::runtime_resource_id::RuntimeResourceID;
use tex_rs::enums::{InterpretAs, RenderFormat, TextureType};
use tex_rs::pack::MipFilter::Linear;
use tex_rs::pack::TextureMapBuilder;
use tex_rs::texture_map::TextureMap;
use tex_rs::WoaVersion;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    //Parameters
    let tga_path = PathBuf::from("./target/texture.tga");
    let text_rrid = RuntimeResourceID::from_hex_string("000210D1CF04E4E4")?;
    let texd_rrid = RuntimeResourceID::from_hex_string("00752CEA9F76AB7E")?;
    let woa_version = WoaVersion::HM3;

    //Input texture
    let text_data = fs::read("./target/0005D89496C3FC78.TEXT")?;
    let texd_data = fs::read("./target/00EFBDEB0ED40D59.TEXD")?;

    let mut reader = Cursor::new(text_data);
    let mut old_texture = TextureMap::read_le_args(&mut reader, (WoaVersion::HM2,))?;
    old_texture.set_mipblock1_data(&texd_data, WoaVersion::HM2)?;


    let partition_id : PartitionId = "chunk12".parse().unwrap();
    let patch_id : PatchId = PatchId::Patch(5);

    let add_texd = true;

    //create a package
    let mut package = PackageBuilder::new_with_patch_id(partition_id, patch_id);

    //Create texture from tga
    let texture =
        TextureMapBuilder::from_texture_map(&old_texture)?
            .interpret_as(InterpretAs::Normal)
            .texture_type(TextureType::Colour)
            .with_mip_filter(Linear)
            .with_mipblock1(add_texd)
            .with_format(RenderFormat::DXT1).build(woa_version)?;

    //Add resources to package
    let mut texture_resource = PackageResourceBuilder::from_glacier_resource(text_rrid, &texture, woa_version.into())?;
    if texture.has_mipblock1(){
        let mipblock1 = texture.get_mipblock1().unwrap();
        let highmip_resource = PackageResourceBuilder::from_glacier_resource(texd_rrid, &mipblock1, woa_version.into())?;
        texture_resource.with_reference(texd_rrid, ResourceReferenceFlags::Standard(ResourceReferenceFlagsStandard::new().with_reference_type(ReferenceType::WEAK)));
        package.with_resource(highmip_resource);
    }
    package.with_resource(texture_resource);

    package.build_to_file(PackageVersion::RPKGv1, "./target".as_ref())?;
    Ok(())
}