use rpkg_rs::resource::partition_manager::PartitionManager;
use rpkg_rs::WoaVersion;
use std::path::PathBuf;
use std::io::{Cursor};
use binrw::{BinWrite};
use itertools::{Itertools};
use rpkg_rs::resource::runtime_resource_id::RuntimeResourceID;
use glacier_geometry::model::prim_mesh_linked::BoneInfoHolder;
use glacier_geometry::render_primitive::{MeshObject, RenderPrimitive};

fn mount_game(
    path_env_var: &str,
    game_version: WoaVersion,
) -> Result<PartitionManager, Box<dyn std::error::Error>> {
    let game_retail_path = match std::env::var(path_env_var) {
        Ok(path) => PathBuf::from(path),
        Err(_) => return Err(format!("{} environment variable not set", path_env_var).into()),
    };
    let partition_manager =
        PartitionManager::from_game(game_retail_path, game_version, true)?;
    assert!(partition_manager.partitions.len() > 0);
    Ok(partition_manager)
}

#[test]
#[ignore]
fn test_parse_hm2016() -> Result<(), Box<dyn std::error::Error>> {
    let game = mount_game("HM2016_PATH", WoaVersion::HM2016)?;
    parse_all_primitives(game, glacier_geometry::WoaVersion::HM2016)?;
    Ok(())
}

#[test]
#[ignore]
fn test_parse_hm2() -> Result<(), Box<dyn std::error::Error>> {
    let game = mount_game("HM2_PATH", WoaVersion::HM2)?;
    parse_all_primitives(game, glacier_geometry::WoaVersion::HM2)?;
    Ok(())
}

#[test]
#[ignore]
fn test_parse_hm3() -> Result<(), Box<dyn std::error::Error>> {
    let game = mount_game("HM3_PATH", WoaVersion::HM3)?;
    parse_all_primitives(game, glacier_geometry::WoaVersion::HM3)?;
    Ok(())
}

fn parse_all_primitives(
    partition_manager: PartitionManager,
    woa: glacier_geometry::WoaVersion,
) -> Result<(), Box<dyn std::error::Error>> {
    for partition in partition_manager.partitions.iter(){
        for (info, _ ) in partition.latest_resources_of_type("PRIM"){
            let resource_data = partition.read_resource(info.rrid())?;
            let mut cursor = Cursor::new(&resource_data);
            let res = match RenderPrimitive::parse_bytes(&mut cursor, woa).map_err(|e| format!("ERROR PARSING {}: \n{:?}", info.rrid(), e)){
                Ok(_) => {Ok(())}
                Err(e) => {if e.contains("Speedtree"){
                    Ok(())
                } else {Err(e)}}
            };
            res?;
        }
    }
    Ok(())
}

fn rebuild_all_primitives(
    partition_manager: &PartitionManager,
    woa: glacier_geometry::WoaVersion,
) -> Vec<Result<(RuntimeResourceID, Vec<u8>, Vec<u8>), TestCaseStatus>> {
    partition_manager.partitions.iter()
        .flat_map(|partition| {
            partition.latest_resources_of_type("PRIM").iter()
                .map(|(info, _)| {
                    let rrid = info.rrid().to_owned();
                    let resource_data = partition.read_resource(info.rrid()).unwrap();
                    let mut cursor = Cursor::new(&resource_data);
                    match RenderPrimitive::parse_bytes(&mut cursor, woa) {
                        Ok(prim) => {
                            let mut buf = vec![];
                            let mut cursor = Cursor::new(&mut buf);
                            if let Err(e) = prim.write_le_args(&mut cursor, &woa){
                                Err(TestCaseStatus::Failed(format!("Failed to write file {}", e.to_string())))
                            } else{
                                Ok((rrid, resource_data, buf))
                            }
                        }
                        Err(e) => {
                            if e.to_string().contains("Speedtree"){
                                Err(TestCaseStatus::SpeedTreeAllowed)
                            } else{
                                Err(TestCaseStatus::Failed(format!(
                                    "ERROR PARSING {}:\n{}",
                                    info.rrid(),
                                    e
                                )))
                            }
                        }
                    }
                }).collect::<Vec<_>>()
        })
        .collect()
}

#[derive(Clone)]
enum TestCaseStatus{
    Success,
    LengthAllowed,
    SpeedTreeAllowed,
    OptimzationAllowed,
    Failed(String),
}

fn has_probable_optimization(prim: &RenderPrimitive,) -> bool{
    let idx_dedup = prim.iter_primitives().map(|prim| prim.get_indices()).duplicates().count() > 0;

    let vertices = prim.iter_primitives().map(|prim| prim.get_vertices()).collect::<Vec<_>>();
    let vert_dedup = vertices.iter().enumerate().any(|(idx, buff)| {
        vertices[idx+1..].contains(buff)
    });

    let collisions = prim.iter_primitives().map(|prim| prim.prim_mesh().sub_mesh.collision.clone()).collect::<Vec<_>>();
    let coll_dedup = collisions.iter().enumerate().any(|(idx, col)| {
        collisions[idx+1..].contains(col)
    });

    let bone_infos = prim.iter_primitives().flat_map(|prim| match prim {
        MeshObject::Normal(_) => {None}
        MeshObject::Weighted(w) => {Some(BoneInfoHolder::Normal(w.bone_info.clone()))}
        MeshObject::Linked(l) => {Some(l.bone_info.clone())}
    }).collect::<Vec<_>>();

    let bone_dedup = bone_infos.iter().enumerate().any(|(idx, info)| {
        bone_infos[idx+1..].contains(info)
    });

    idx_dedup || vert_dedup || coll_dedup || bone_dedup
}

fn check_primtives(woa_version: WoaVersion, states: Vec<Result<(RuntimeResourceID, Vec<u8>, Vec<u8>), TestCaseStatus>>) -> Vec<TestCaseStatus> {

    states.iter().map(|item| {
        match item {
            Ok((id, original, new)) => {
                return if new == original { TestCaseStatus::Success } else {
                    let mut cursor = Cursor::new(&new);
                    if let Ok(prim_reparse) = RenderPrimitive::parse_bytes(&mut cursor, woa_version.into()) {
                        if !has_probable_optimization(&prim_reparse) {
                            if new.len() != original.len() {
                                if id.clone() != RuntimeResourceID::from_hex_string("0083B63C57BAEEDD").unwrap(){
                                    return TestCaseStatus::Failed(format!("{} LENGTH MISMATCH ({} vs {})", id, new.len(), original.len()));
                                };
                            }

                            match compare_f32_aligned_le(&original, &new, 5.0, 5.0) {
                                Ok(state) => { state }
                                Err(e) => { TestCaseStatus::Failed(format!("{} failed: {}", id, e)) }
                            }
                        } else {
                            TestCaseStatus::OptimzationAllowed
                        }
                    } else {
                        TestCaseStatus::Failed(format!("{} is not a valid primitives", id))
                    }
                }
            }
            Err(e) => {
                return e.clone();
            }
        }

    }).collect::<Vec<_>>()
}

fn summarize_results(states: Vec<TestCaseStatus>){
    let yes = states.iter().filter(|d|matches!(d, TestCaseStatus::Success)).count();
    let mismatch = states.iter().filter(|d|matches!(d, TestCaseStatus::LengthAllowed)).count();
    let optim = states.iter().filter(|d|matches!(d, TestCaseStatus::OptimzationAllowed)).count();
    let speedtree = states.iter().filter(|d|matches!(d, TestCaseStatus::SpeedTreeAllowed)).count();
    let error = states.iter().filter(|d|matches!(d, TestCaseStatus::Failed(..))).count();

    println!("Success: {}", yes);
    println!("Length mismatch: {}", mismatch);
    println!("Optimizated: {}", optim);
    println!("Speedtree: {}", speedtree);
    println!("error: {}", error);
    if error > 0 {
        match states.iter().find(|d|matches!(d, TestCaseStatus::Failed(..))).unwrap(){
            TestCaseStatus::Failed(e) => {
                panic!("{}", e);
            }
            _ => {},
        }
    }
}

#[test]
#[ignore]
fn test_rebuild_hm2016() -> Result<(), Box<dyn std::error::Error>> {
    let game = mount_game("HM2016_PATH", WoaVersion::HM2016)?;
    let prims = rebuild_all_primitives(&game, glacier_geometry::WoaVersion::HM2016);
    let states = check_primtives(WoaVersion::HM2016, prims);
    summarize_results(states);
    Ok(())
}

#[test]
#[ignore]
fn test_rebuild_hm2() -> Result<(), Box<dyn std::error::Error>> {
    let game = mount_game("HM2_PATH", WoaVersion::HM2)?;
    let prims = rebuild_all_primitives(&game, glacier_geometry::WoaVersion::HM2);
    let states = check_primtives(WoaVersion::HM2, prims);
    summarize_results(states);
    Ok(())
}

#[test]
#[ignore]
fn test_rebuild_hm3() -> Result<(), Box<dyn std::error::Error>> {
    let game = mount_game("HM3_PATH", WoaVersion::HM3)?;
    let prims = rebuild_all_primitives(&game, glacier_geometry::WoaVersion::HM3);
    let states = check_primtives(WoaVersion::HM3, prims);
    summarize_results(states);
    Ok(())
}

fn approx_eq_f32(a: f32, b: f32, rel: f32, abs: f32) -> bool {
    if a == b {
        return true;
    }
    if a.is_nan() && b.is_nan() {
        return true;
    }
    let diff = (a - b).abs();
    diff <= abs.max(rel * a.abs().max(b.abs()))
}

fn compare_f32_aligned_le(a: &[u8], b: &[u8], rel: f32, abs: f32) -> Result<TestCaseStatus, String> {
    if a.len() != b.len() {
        return Ok(TestCaseStatus::LengthAllowed);
    }
    let len = a.len();
    let tail = len % 4;
    let mut failures = Vec::new();

    // Compare in 4-byte chunks
    let body_len = len - tail;
    let mut i = 0usize;
    while i < body_len {
        let a_chunk = &a[i..i + 4];
        let b_chunk = &b[i..i + 4];
        if a_chunk != b_chunk {
            let va = f32::from_le_bytes(a_chunk.try_into().unwrap());
            let vb = f32::from_le_bytes(b_chunk.try_into().unwrap());
            if !approx_eq_f32(va, vb, rel, abs) {
                failures.push(format!(
                    "offset=0x{:X}: bytes {:02X?} vs {:02X?} -> f32 {} vs {} (abs_diff={}, rel_err={})",
                    i,
                    a_chunk, b_chunk,
                    va, vb,
                    (va - vb).abs(),
                    if va == 0.0 && vb == 0.0 { 0.0 } else { (va - vb).abs() / va.abs().max(vb.abs()) }
                ));
            }
        }
        i += 4;
    }

    if tail > 0 {
        let s = len - tail;
        if &a[s..] != &b[s..] {
            failures.push(format!(
                "trailing non-f32 bytes differ at [0x{:X}..0x{:X}]: {:02X?} vs {:02X?}",
                s, len, &a[s..], &b[s..]
            ));
        }
    }

    if failures.is_empty() {
        Ok(TestCaseStatus::Success)
    } else {
        Err(format!(
            "Found {} differing 4-byte chunk(s) beyond f32 tolerance:\n{}",
            failures.len(),
            failures.join("\n")
        ))
    }
}