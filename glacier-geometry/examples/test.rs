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
use itertools::Itertools;

fn generate_color(i: usize) -> [u8; 3] {
    let r = ((i * 97) % 256) as u8; // A pseudo-random pattern for red
    let g = ((i * 67) % 256) as u8; // A pseudo-random pattern for green
    let b = ((i * 43) % 256) as u8; // A pseudo-random pattern for blue
    [r, g, b]
}

fn test_bone_groups(model: RenderPrimitive, file_path: PathBuf){

    for (i, primitive) in model.iter_primitive_of_lod(LEVEL8).enumerate() {
        let filename = format!("./target/output{}.ply", i);

        let index_groups = (0..1000).map(|i| match primitive {
            MeshObject::Normal(_) => {None}
            MeshObject::Weighted(_) => {None}
            MeshObject::Linked( linked ) => {
                linked.get_indices_for_bone(i)
            }
        }).flatten().collect::<Vec<_>>();

        let mut colors: Vec<[u8; 3]> =
            vec![[255, 255, 255]; primitive.get_positions().len()];

        for (i, index_group) in index_groups.iter().enumerate() {
            for index in index_group {
                colors[*index as usize] = generate_color(i);
            }
        }

        write_ply_with_colors(
            &primitive.get_positions(),
            &colors,
            Some(&primitive.get_indices()),
            filename.as_str(),
        )
            .unwrap()
    }
}

fn convert_to_ply(model: RenderPrimitive, file_path: PathBuf, stats_set: &mut HashSet<String>){
    for primitive in model.iter_primitives() {
        match primitive {
            MeshObject::Normal(_) => {}
            MeshObject::Weighted(prim) => {}
            MeshObject::Linked(prim) => {
                // println!("{}", file_path.file_name().unwrap().to_str().unwrap());
                // if(prim.unk_data.unk3 != 0){
                //     println!("Unkown triangle count: {}", prim.unk_data.unk3);
                // }

                // if (flags.has_bones()){
                //     println!("bruh");
                // }

                // prim.unk_data.unk2 == 0 && prim.unk_data.unk3 != 0 | found, unk2 and 3 are not a u64
                // prim.unk_data.unk1 as usize > primitive.get_indices().len() | found, unk1 is not a idx index
                let mut binary_str = String::new();

                if (prim.bone_info.total_chunks_align
                    != (prim.bone_info.bone_remap.get_ref().len()) as u32)
                {
                    println!("{:?}", prim.bone_info.bone_remap.get_ref());

                    println!(
                        "Bone remap broken ({} != {})",
                        prim.bone_info.total_chunks_align,
                        prim.bone_info.bone_remap.get_ref().len()
                    );
                    // exit(0);
                }

                for i in prim.bone_info.bone_remap.clone().into_bit_vec().iter() {
                    binary_str += format!("{}", if i { "1" } else { "0" }).as_str();
                }

                //num_blocks is never larger than total_chunks_align
                if binary_str.chars().filter(|char| *char == '1').count()
                    != prim.bone_info.num_blocks as usize
                {
                    println!("{}", file_path.file_name().unwrap().to_str().unwrap());
                    println!(
                        "{} - {}",
                        binary_str.chars().filter(|char| *char == '1').count(),
                        prim.bone_info.num_blocks as usize
                    );

                    let split = binary_str.split_at(
                        binary_str.len() - prim.bone_info.total_chunks_align as usize,
                    );

                    stats_set.insert(format!(
                        "{}\n1: {}\n2: {}{}\n",
                        format!(
                            "[num: {} total: {}]",
                            prim.bone_info.total_chunks_align,
                            binary_str.len()
                        ),
                        binary_str,
                        split.0.chars().into_iter().map(|_| 'x').collect::<String>(),
                        split.1
                    ));
                }

                // stats_set.insert(format!("({},{})", prim.unk_data.total_chunks_align, prim.unk_data.num_blocks));
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from("/media/dafitius/980 PRO/HitmanProjects/Tools/rpkgtools2.24/allPrim");
    let mut stats_set: HashSet<String> = HashSet::new();

    for entry in read_dir(&path)? {
        let entry = entry?;
        let file_path = entry.path();

        if file_path.is_file() {
            let mut data = Cursor::new(fs::read(&file_path)?);
            match RenderPrimitive::parse_bytes(&mut data) {
                Ok(model) => {
                    // if !model.flags().is_weighted_object() || fs::read(&file_path)?.len() < 2_000_000 {
                    //     continue;
                    // }
                    // test_bone_groups(model, file_path);
                    // break;

                    // convert_to_ply(model, file_path, &mut stats_set);

                    model.iter_primitives().for_each(|primitive| {
                        match primitive {
                            MeshObject::Normal(_) => {}
                            MeshObject::Weighted(_) => {}
                            MeshObject::Linked(_) => {}
                        }
                    })
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

    for stat in &stats_set {
        println!("{}", stat);
    }

    Ok(())
}

fn write_obj(
    positions: &Vec<Vector4>,
    normals: &Vec<Vector4>,
    tex_coords: &Vec<Vector2>,
    indices: &Vec<u16>,
    filename: &str,
) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let mut writer = BufWriter::new(file);

    // Write vertex positions (v)
    for pos in positions {
        // Ignore w component
        writeln!(writer, "v {} {} {}", pos.x, pos.y, pos.z)?;
    }

    // Write texture coordinates (vt)
    for tc in tex_coords {
        writeln!(writer, "vt {} {}", tc.x, tc.y)?;
    }

    // Write normals (vn)
    for n in normals {
        writeln!(writer, "vn {} {} {}", n.x, n.y, n.z)?;
    }

    // Write faces (f)
    // Assuming that positions, tex_coords, and normals all match 1-to-1 and that
    // each triple of indices forms a triangle.
    for face in indices.chunks(3) {
        if face.len() == 3 {
            let v1 = face[0] as usize + 1;
            let v2 = face[1] as usize + 1;
            let v3 = face[2] as usize + 1;

            // f v/tc/n for each vertex
            // Assuming a 1:1 correspondence of indices for v, vt, vn
            writeln!(writer, "f {0}/{0}/{0} {1}/{1}/{1} {2}/{2}/{2}", v1, v2, v3)?;
        }
    }

    Ok(())
}

fn write_ply_with_colors(
    positions: &Vec<Vector4>,
    colors: &Vec<[u8; 3]>, // RGB colors for each vertex
    indices: Option<&Vec<u16>>,
    filename: &str,
) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let mut writer = BufWriter::new(file);

    // Header
    writeln!(writer, "ply")?;
    writeln!(writer, "format ascii 1.0")?;
    writeln!(writer, "element vertex {}", positions.len())?;
    writeln!(writer, "property float x")?;
    writeln!(writer, "property float y")?;
    writeln!(writer, "property float z")?;
    writeln!(writer, "property uchar red")?;
    writeln!(writer, "property uchar green")?;
    writeln!(writer, "property uchar blue")?;
    if let Some(indices) = indices {
        writeln!(writer, "element face {}", indices.len() / 3)?;
        writeln!(writer, "property list uchar int vertex_indices")?;
    }
    writeln!(writer, "end_header")?;

    // Write vertex data
    for (pos, color) in positions.iter().zip(colors.iter()) {
        writeln!(
            writer,
            "{} {} {} {} {} {}",
            pos.x, pos.y, pos.z, color[0], color[1], color[2]
        )?;
    }

    // Write face data
    if let Some(indices) = indices {
        for face in indices.chunks(3) {
            if face.len() == 3 {
                writeln!(
                    writer,
                    "3 {} {} {}",
                    face[0] as usize, face[1] as usize, face[2] as usize
                )?;
            }
        }
    }

    Ok(())
}
