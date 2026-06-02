use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use directxtex::{ScratchImage, CP_FLAGS_NONE, DDS_FLAGS, DDS_FLAGS_NONE, DXGI_FORMAT_BC6H_UF16, DXGI_FORMAT_R8G8B8A8_UNORM, TEX_FILTER_BOX, TEX_THRESHOLD_DEFAULT};
use image::{DynamicImage, Rgba32FImage};
use image::codecs::openexr::OpenExrDecoder;
use rpkg_rs::resource::package_builder::{PackageBuilder, PackageResourceBuilder};
use rpkg_rs::resource::pdefs::PartitionId;
use rpkg_rs::resource::resource_package::{PackageVersion, ReferenceType, ResourceReferenceFlags, ResourceReferenceFlagsStandard};
use rpkg_rs::resource::resource_partition::PatchId;
use rpkg_rs::resource::runtime_resource_id::RuntimeResourceID;
use glacier_base::math::Vector3;
use glacier_texture::box_reflection::{BoxReflection, BoxReflectionCache, CubemapLayout};
use glacier_texture::image::BoxReflectionDecoder;
use glacier_texture::texture_map::TextureMap;
use glacier_texture::WoaVersion;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let boxc = BoxReflectionCache::from_file("/path/to/00ABCDEF01234567.BOXC")?;
    println!("Boxc loaded with {:?} entries", boxc.num_box_reflections());

    let out_dir = PathBuf::from("/target/box");
    let layout = CubemapLayout::HorizontalCross;
    
    for (i, boxr) in boxc.iter().take(1).enumerate() {
        let dec = BoxReflectionDecoder::from_box_reflection(boxr.clone(), layout);
        let image = DynamicImage::from_decoder(dec)?;
        image.save(out_dir.join( format!("decoded-{}.exr", i)))?;
    }

    let boxc = out_dir.read_dir()?
        .filter_map(Result::ok)
        .filter(|e|e.metadata().unwrap().is_file())
        .flat_map(|file| -> Result<BoxReflection, Box<dyn std::error::Error>> {
            let data = fs::read(file.path()).unwrap();
            let cursor = Cursor::new(data);
            let image = DynamicImage::from_decoder(OpenExrDecoder::new(cursor)?)?;
            Ok(BoxReflection::from_dynamic_image(&image, Vector3::default())?)
    }).collect::<BoxReflectionCache>();

    boxc.pack_to_file(out_dir.join("out.BOXC"))?;

    let new_boxc = BoxReflectionCache::from_file(out_dir.join("out.BOXC"))?;
    for (i, boxr) in new_boxc.iter().enumerate() {
        let dec = BoxReflectionDecoder::from_box_reflection(boxr.clone(), layout);
        let image = DynamicImage::from_decoder(dec)?;
        image.save(out_dir.join(format!("decoded_again-{}.exr", i)))?;
    }
    Ok(())
}
