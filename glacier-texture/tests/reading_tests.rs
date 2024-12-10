use std::io::{Cursor};
use std::path::{Path, PathBuf};
use std::process::exit;
use binrw::{BinRead, Error};
use directxtex::{DDS_FLAGS, TEX_COMPRESS_FLAGS, TEX_FILTER_FLAGS, TEX_THRESHOLD_DEFAULT};
use rpkg_rs::GlacierResourceError;
use rpkg_rs::resource::partition_manager::PartitionManager;
use rpkg_rs::resource::pdefs::{GamePaths, PackageDefinitionSource};

use tex_rs::texture_map::{TextureMap};
use tex_rs::{convert, WoaVersion};
use tex_rs::pack::TexturePacker;

#[test]
#[ignore]
fn read_all() {
    read_all_in_h2016();
    read_all_in_h2();
    read_all_in_h3();
}

#[test]
#[ignore]
fn read_all_in_h2016() {
    let game_version = rpkg_rs::WoaVersion::HM2016;
    // Discover the game paths.
    let game_paths = GamePaths {
        project_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/Hitman™/"),
        runtime_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/Hitman™/Runtime/"),
        package_definition_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/Hitman™/Runtime/packagedefinition.txt"),
    };
    read_all_text_texd_in_game(game_version, game_paths);
}

#[test]
#[ignore]
fn read_all_in_h2() {
    let game_version = rpkg_rs::WoaVersion::HM2;
    // Discover the game paths.
    let game_paths = GamePaths {
        project_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/HITMAN2/"),
        runtime_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/HITMAN2/Runtime/"),
        package_definition_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/HITMAN2/Runtime/packagedefinition.txt"),
    };
    read_all_text_texd_in_game(game_version, game_paths);
}

#[test]
#[ignore]
fn read_all_in_h3() {
    let game_version = rpkg_rs::WoaVersion::HM3;
    // Discover the game paths.
    let game_paths = GamePaths {
        project_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/HITMAN 3/"),
        runtime_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/HITMAN 3/Runtime/"),
        package_definition_path: PathBuf::from("/media/dafitius/980 PRO/SteamLibrary/steamapps/common/HITMAN 3/Runtime/packagedefinition.txt"),
    };
    read_all_text_texd_in_game(game_version, game_paths);
}

fn read_all_text_texd_in_game(woa_version: rpkg_rs::WoaVersion, game_paths: GamePaths) {

    // Read and parse the package definition.
    let package_definition_source =
        PackageDefinitionSource::from_file(game_paths.package_definition_path, woa_version)
            .unwrap_or_else(|e| {
                eprintln!("failed to parse package definition: {}", e);
                std::process::exit(0);
            });

    let mut partition_infos = package_definition_source.read().unwrap_or_else(|e| {
        eprintln!("failed to read package definition: {}", e);
        std::process::exit(0);
    });

    // Ignore modded patches.
    for partition in partition_infos.iter_mut() {
        partition.set_max_patch_level(9);
    }

    let mut package_manager =
        PartitionManager::new(game_paths.runtime_path, &package_definition_source).unwrap_or_else(
            |e| {
                eprintln!("failed to init package manager: {}", e);
                std::process::exit(0);
            },
        );


    package_manager
        .mount_partitions(|_, _| {})
        .unwrap_or_else(|e| {
            eprintln!("failed to mount partitions: {}", e);
            std::process::exit(0);
        });

    println!("Finished mounting game");

    for partition in &package_manager.partitions {
        println!("start reading {}", partition.partition_info().id());
        for (resource, id) in partition.latest_resources() {
            if (resource.data_type() == "TEXT") {
                let raw_data = package_manager.read_resource_from(partition.partition_info().id(), *resource.rrid()).map_err(|e| GlacierResourceError::ReadError(e.to_string())).unwrap();
                let mut stream = Cursor::new(raw_data);
                match TextureMap::read_le_args(&mut stream, (WoaVersion::from(woa_version),)) {
                    Ok(mut texture_map) => {
                        if let Some(texd_ref) = resource.references().get(0) {
                            let texd_data = package_manager.read_resource_from(partition.partition_info().id(), texd_ref.0).map_err(|e| GlacierResourceError::ReadError(e.to_string())).unwrap();
                            texture_map.set_mipblock1_raw(&texd_data, woa_version.into()).unwrap();
                        }
                    }
                    Err(e) => {
                        match e {
                            _ => {
                                let message = e.to_string();
                                if message.contains("Looks like you tried to export a cubemap texture, those are not supported yet at 0x0") {
                                    continue;
                                }
                                panic!("{}", e)
                            }
                        }
                    }
                }
            }
        }
    }
}