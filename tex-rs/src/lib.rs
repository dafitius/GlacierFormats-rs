use std::io;
use std::io::Cursor;
use std::str::FromStr;
use binrw::{BinRead, BinWrite};
use rpkg_rs::{GlacierResource, GlacierResourceError};
use crate::texture_map::TextureMap;

pub mod texture_map;

#[cfg(feature = "rpkg")]
pub mod rpkg;
pub mod convert;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum WoaVersion {
    HM2016,
    HM2,
    HM3,
}

impl FromStr for WoaVersion {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "H1" | "HM1" | "HM2016" | "H2016" => Ok(WoaVersion::HM2016),
            "2" | "H2" | "HM2" | "HM2018" | "H2018" => Ok(WoaVersion::HM2),
            "3" | "H3" | "HM3" | "HM2020" | "H2020" => Ok(WoaVersion::HM3),
            _ => Err(format!("Invalid value for WoaVersion: {}", s)),
        }
    }
}

pub trait Version {
    fn get_version() -> WoaVersion;
}
