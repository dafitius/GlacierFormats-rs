use glacier_base::math::Vector3;
use glacier_texture::box_reflection::{BoxReflection, BoxReflectionCache, CubemapLayout};
use glacier_texture::image::BoxReflectionDecoder;
use image::codecs::openexr::OpenExrDecoder;
use image::DynamicImage;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let boxc = BoxReflectionCache::from_file("/path/to/00ABCDEF01234567.BOXC")?;
    println!("Boxc loaded with {:?} entries", boxc.len());

    let out_dir = PathBuf::from("/target/box");
    let layout = CubemapLayout::HorizontalCross;

    for (i, boxr) in boxc.iter().take(1).enumerate() {
        let image = boxr.create_dynamic_image(layout)?;
        image.save(out_dir.join(format!("decoded-{}.exr", i)))?;
    }

    let boxc = out_dir
        .read_dir()?
        .filter_map(Result::ok)
        .filter(|e| e.metadata().unwrap().is_file())
        .flat_map(
            |file| -> Result<BoxReflection, Box<dyn std::error::Error>> {
                let data = fs::read(file.path()).unwrap();
                let cursor = Cursor::new(data);
                let image = DynamicImage::from_decoder(OpenExrDecoder::new(cursor)?)?;
                Ok(BoxReflection::from_dynamic_image(
                    &image,
                    Vector3::default(),
                )?)
            },
        )
        .collect::<BoxReflectionCache>();

    boxc.pack_to_file(out_dir.join("out.BOXC"))?;

    let new_boxc = BoxReflectionCache::from_file(out_dir.join("out.BOXC"))?;
    for (i, boxr) in new_boxc.iter().enumerate() {
        let dec = BoxReflectionDecoder::from_box_reflection(boxr.clone(), layout);
        let image = DynamicImage::from_decoder(dec)?;
        image.save(out_dir.join(format!("decoded_again-{}.exr", i)))?;
    }
    Ok(())
}
