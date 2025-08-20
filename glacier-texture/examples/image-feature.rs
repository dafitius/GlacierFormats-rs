use std::env;
use glacier_texture::enums::{RenderFormat, TextureType};
use glacier_texture::image::{TextureMapDecoder, TextureMapEncoder};
use glacier_texture::mipblock::MipblockData;
use glacier_texture::pack::{TextureMapBuilder, TextureMapParameters};
use glacier_texture::texture_map::TextureMap;
use glacier_texture::{convert, WoaVersion};
use image::{DynamicImage};
use std::io::{BufReader, BufWriter, Cursor};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_image> <output_image> [--high-level]", args[0]);
        std::process::exit(1);
    }
    let input_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);
    let use_high_level = args.get(3).map_or(false, |s| s == "--high-level");

    if use_high_level {
        println!("Using high-level API");
        main_high_level(input_path, output_path)
    } else {
        println!("Using low-level API");
        main_low_level(input_path, output_path)
    }
}

fn main_low_level(input_path: PathBuf, output_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    //Set up resource buffers
    let mut text = vec![];
    let mut texd = vec![];

    //Set up writers
    let text_writer = BufWriter::new(Cursor::new(&mut text));
    let texd_writer = BufWriter::new(Cursor::new(&mut texd));

    //read image using image-rs
    let jpeg = image::open(input_path)?;

    //Set up texture parameters
    let mut params = TextureMapParameters::new(RenderFormat::BC7);
    params.set_texture_type(TextureType::Colour);

    //encode image to TEXT and TEXD buffers
    let enc = TextureMapEncoder::new(
        text_writer,
        Some(texd_writer),
        WoaVersion::HM3,
        Some(params),
        None,
    );
    jpeg.write_with_encoder(enc)?;

    //read buffers into texture_map
    let mut texture_map = TextureMap::from_memory(&*text, WoaVersion::HM3)?;
    texture_map.set_mipblock1(MipblockData::from_memory(&*texd, WoaVersion::HM3)?);

    //Set up readers
    let text_reader = BufReader::new(Cursor::new(text));
    let texd_reader = BufReader::new(Cursor::new(texd));

    let dec = TextureMapDecoder::new(text_reader, Some(texd_reader), WoaVersion::HM3);
    let image = DynamicImage::from_decoder(dec)?;
    image.save(output_path)?;
    Ok(())
}

fn main_high_level(input_path: PathBuf, output_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    
    //read the image
    let jpeg = image::open(input_path)?;

    //create TextureMap from the image
    let texture_map = TextureMapBuilder::from_dynamic_image(jpeg)?
        .with_format(RenderFormat::BC7)
        .with_texture_type(TextureType::Colour)
        .build(WoaVersion::HM3)?;
    
    //create and save image
    let image = convert::create_dynamic_image(&texture_map)?;
    image.save(output_path)?;
    Ok(())
}
