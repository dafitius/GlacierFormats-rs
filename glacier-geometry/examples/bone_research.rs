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
use glacier_geometry::model::prim_mesh_linked::PrimMeshLinked;

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from("/media/dafitius/980 PRO/HitmanProjects/Tools/rpkgtools2.24/allPrim");
    // let mut set = HashSet::new();
    for entry in read_dir(&path)?
        .filter(|entry| entry.is_ok())
        .sorted_by(|entry, entry2| {
            let e = entry.as_ref().unwrap().metadata().unwrap().len();
            let e2 = entry2.as_ref().unwrap().metadata().unwrap().len();
            e.cmp(&e2)
        }) {
        let entry = entry?;
        let file_path = entry.path();
        // if !file_path.file_name().unwrap().to_str().unwrap().to_string().contains("006BE350D6F984A0"){
        //     continue;
        // }
        if file_path.is_file() {
            let mut data = Cursor::new(fs::read(&file_path)?);
            match RenderPrimitive::parse_bytes(&mut data) {
                Ok(model) => {
                    if !model.flags().is_weighted_object(){
                        continue;
                    }
                    
                    for prim in model.iter_primitives() {
                        if let MeshObject::Weighted(mesh) = prim {
                            let mut indices = HashSet::new();
                            println!("total indices: {} max: {}", mesh.prim_mesh.get_indices().len() , mesh.prim_mesh.get_indices().iter().max().unwrap());

                            println!("bone remap: {:?}", mesh.bone_info.clone().bone_remap.iter().enumerate().filter(|(i, &idx)| idx != 255).map(|(i, idx)| format!("{}: {}", i, idx)).collect::<Vec<String>>());
                            println!("accel entry count: {:?}", mesh.bone_info.accel_entries.len());

                            let bone_indices = mesh.bone_indices.clone();
                            println!("bone_indices idx size: {:?}, max value: {:?}", bone_indices.indices.len(), bone_indices.indices.iter().max().unwrap());


                            if let Some(copy_bones) = mesh.copy_bones.clone(){
                                println!("copy bones idx size: {:?}, max value: {:?}", copy_bones.indices.len(), copy_bones.indices.iter().max().unwrap());
                                println!("copy bones offset size: {:?}, max value: {:?}", copy_bones.offsets.len(), copy_bones.offsets.iter().max().unwrap());
                            }

                            println!("bone indices: {:?}", mesh.bone_indices);
                            println!("accel entries: {:?}", mesh.bone_info.clone().accel_entries);
                            if let Some(weights) = mesh.prim_mesh.clone().get_weights(){
                                for (i, weight) in weights.iter().enumerate(){

                                    for idx in weight.indices{
                                        if idx == 0 {
                                            continue;
                                        }

                                        indices.insert(idx);
                                        if &mesh.bone_info.clone().bone_remap[idx as usize] == &0xff {

                                            // println!("ff found");
                                        }else{
                                            let bone_info = mesh.bone_info.clone();
                                            let accel_idx = bone_info.bone_remap[idx as usize];
                                            let accel_entry = &bone_info.accel_entries[accel_idx as usize];
                                            let accel_slice = mesh.bone_indices.indices.iter().skip(accel_entry.offset as usize).take(accel_entry.num_indices as usize).collect_vec();
                                            println!("{}. idx: {} item: {} accel: {:?}",i, idx, &accel_idx, accel_slice);

                                        }
                                    }
                                }
                            }

                        }
                        // exit(1)
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
    //
    // for item in set {
    //     println!("{:08x}", item);
    // }

    Ok(())
}

