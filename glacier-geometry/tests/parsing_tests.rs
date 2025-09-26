use rpkg_rs::resource::partition_manager::PartitionManager;
use rpkg_rs::WoaVersion;
use std::path::PathBuf;
use std::io::Cursor;
use glacier_geometry::render_primitive::RenderPrimitive;

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
