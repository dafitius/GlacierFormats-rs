use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::process::exit;
use binrw::{BinRead, BinWrite, Endian};
use walkdir::WalkDir;
use convert::create_tga;
use tex_rs::texture_map::{RenderFormat, TextureMap, TextureType};
use tex_rs::{convert, WoaVersion};
use tex_rs::convert::TextureConversionError;
use tex_rs::texture_map::TextureType::Normal;

//"/home/dafitius/Documents/Hitman modding/Hitman files/chunk0/TEXT/00F3D677AE94EA21.TEXT"

fn main() {
    // let folder = Path::new("/media/dafitius/980 PRO/HitmanProjects/Tools/rpkgtools2.33/H1L-TEXT-ALL/");
    // let woa_version = WoaVersion::HM2016;
    //
    // let folder = Path::new("/home/dafitius/Documents/Hitman modding/Tools/rpkgtools2.33/H2-TEXT-ALL/");
    // let woa_version = WoaVersion::HM2;

    let folder = Path::new("/media/dafitius/980 PRO/HitmanProjects/Tools/rpkgtools2.33/H3-TEXT-ALL/");
    let woa_version = WoaVersion::HM3;

    for entry in WalkDir::new(folder) {
        let entry = entry.unwrap();
        let path = entry.path();

        // Check if the file has the desired extension
        if path.is_file() && path.extension().is_some() {
            let extension = path.extension().unwrap().to_str().unwrap();
            if extension == "TEXT" {

                let mut stream = Cursor::new(fs::read(&entry.path()).unwrap());
                if let Ok(tex) = TextureMap::read_le_args(&mut stream, (woa_version, )){
                    match tex.get_header().interpret_as {
                        TextureType::Projection => {
                            println!("({:?},{:?}) on: {:?}", tex.get_header().type_, tex.get_header().interpret_as, entry.path().file_name().unwrap());
                        },
                        _ => {},
                    }
                    // match tex.get_that_one_thing() {
                    //     0 => {},
                    //     x => {
                    //         println!("tag: {} with ({:?},{:?}) on: {:?}", x, tex.get_header().type_, tex.get_header().interpret_as, entry.path().file_name().unwrap());
                    //         let tga = convert::create_tga(&tex).unwrap();
                    //         fs::write(Path::new(&format!("./{}/{:?}.tga", x, entry.path().file_name().unwrap().to_str().unwrap())), tga).expect("TODO: panic message");
                    //     }
                    // }

                    match tex.get_header().interpret_as {
                        TextureType::UNKNOWN64 => {
                            let tga = convert::create_tga(&tex).unwrap();
                            fs::write(Path::new(&format!("./64/{:?}.tga", entry.path().file_name().unwrap())), tga).expect("TODO: panic message");
                        },
                        _ => {},
                    }

                    let mut bytes = Cursor::new(vec![]);
                    tex.write_options(&mut bytes, Endian::Little, ()).unwrap();
                    let original = fs::read(&entry.path()).unwrap();
                    let safe = compare_byte_arrays(original.as_slice(), bytes.get_mut().as_slice());
                    if !safe {
                        println!("for {}", entry.path().display());
                        println!("{:?}", tex.get_header().type_ )
                    }

                }
            }
        }
    }



}

fn compare_byte_arrays(arr1: &[u8], arr2: &[u8]) -> bool {
    // Step 1: Compare sizes
    if arr1.len() != arr2.len() {
        println!("Arrays have different sizes: {} vs {}", arr1.len(), arr2.len());
        return false; // Exit early
    }

    let mut ret_safe = true;
    // Step 2: Find differing bytes
    let mut differing_bytes = Vec::new();
    for (pos, (byte1, byte2)) in arr1.iter().zip(arr2.iter()).enumerate().skip(10) {
        if byte1 != byte2 {
            differing_bytes.push((*byte1, *byte2));
        } else if !differing_bytes.is_empty() {
            println!("Differing bytes: {:?} at {}", differing_bytes, pos - differing_bytes.len());
            differing_bytes.clear();
            ret_safe = false;
        }
    }

    // Print any remaining differing bytes
    if !differing_bytes.is_empty() {
        println!("Differing bytes: {:?} at {}", differing_bytes, arr1.len() - differing_bytes.len());
        ret_safe = false;
    }
    return ret_safe;
}