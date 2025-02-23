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
use glacier_geometry::cloth::cloth::ClothSimMesh;

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from("/media/dafitius/980 PRO/HitmanProjects/Tools/rpkgtools2.24/allPrim");
    let mut set = HashSet::new();
    for entry in read_dir(&path)? {
        let entry = entry?;
        let file_path = entry.path();
        // if !file_path.file_name().unwrap().to_str().unwrap().to_string().contains("006BE350D6F984A0"){
        //     continue;
        // }
        if file_path.is_file() {
            let mut data = Cursor::new(fs::read(&file_path)?);
            match RenderPrimitive::parse_bytes(&mut data) {
                Ok(model) => {
                    // if model.flags().is_weighted_object() || model.flags().is_linked_object() {
                    //     continue;
                    // }

                    let sizes = model.iter_primitives().map(|prim| match &prim.prim_mesh().sub_mesh.cloth_data{
                        None => {None}
                        Some(cloth) => {
                            match cloth {
                                ClothSimMesh::Skinned(_) => {None}
                                ClothSimMesh::Simulation(pack) => {Some(pack.unknown.clone())}
                            }
                        }
                    }).flatten().collect_vec();
                    for size in sizes {
                        for unk in size{
                            set.insert(unk.unk1);
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

    for item in set {
        println!("{:08x}", item);
    }

    Ok(())
}

