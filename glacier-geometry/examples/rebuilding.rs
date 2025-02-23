use glacier_geometry::render_primitive::LodLevel::LEVEL8;
use glacier_geometry::render_primitive::{LodLevel, MeshObject, RenderPrimitive};
use glacier_geometry::utils::math::{Vector2, Vector4};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::fs::{read_dir, File};
use std::io::Write;
use std::io::{BufWriter, Cursor};
use std::path::PathBuf;
use std::process::exit;
use binrw::BinWrite;
use itertools::Itertools;
use rpkg_rs::{GlacierResource, WoaVersion};

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from("/media/dafitius/980 PRO/HitmanProjects/Tools/rpkgtools2.24/allPrim");

    for entry in read_dir(&path)? {
        let entry = entry?;
        let file_path = entry.path();
        if !file_path.file_name().unwrap().to_str().unwrap().to_string().contains("00E942263068F4AE"){
            continue;
        }
        if file_path.is_file() {
            let mut data = Cursor::new(fs::read(&file_path)?);
            match RenderPrimitive::parse_bytes(&mut data) {
                Ok(model) => {
                    if !model.flags().is_linked_object() {
                        continue;
                    }

                    let mut writer = Cursor::new(Vec::new());
                    let opt_read = model.write_le_args(&mut writer, ());
                    match opt_read {
                        Ok(read) => {
                            for prim in model.iter_primitives(){
                                println!("{:?}", prim);
                            }

                            let in_bytes = fs::read(&file_path)?;
                            let out_bytes = writer.into_inner();
                            fs::write("./target/in.bin", in_bytes.clone())?;
                            fs::write("./target/out.bin", out_bytes.clone())?;

                            println!("equal length:    {}", if in_bytes.len() == out_bytes.len() { "✅" } else { "❌" });
                            println!("bytes equal:     {}", if in_bytes == out_bytes { "✅" } else { "❌" });

                            let mut out_rebuild = Cursor::new(out_bytes.clone());
                            match RenderPrimitive::parse_bytes(&mut out_rebuild) {
                                Ok(model_rebuilt) => {
                                    println!("reparsed equal:  {}", if model_rebuilt == model { "✅" } else { "❌" });
                                    if model_rebuilt != model {
                                        println!("rebuild unsuccessful, reparsing results in different model");
                                        exit(1)
                                    }

                                    //getting the sizes to match requires optimizing out duplicate buffers
                                    // if in_bytes.len() != out_bytes.len() {
                                    //     println!("rebuild unsuccessful, sizes differ");
                                    //     exit(1)
                                    // }
                                }
                                Err(e) => {
                                    println!("ERROR: could not read {:?}", file_path);
                                    println!("rebuild failed: {}", e);
                                }
                            }
                            // exit(0)
                        }
                        Err(e) => {
                            println!("{}", e);
                        }
                    }
                }
                Err(e) => {
                    if e.to_string().contains("Speedtree") {
                        continue
                    }

                    println!("ERROR: could not read {:?}", file_path);
                    println!("{:?}", e);
                    exit(1);
                }
            }
        }
    }

    Ok(())
}

