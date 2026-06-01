use std::str::FromStr;

pub mod texture_map;

#[cfg(feature = "rpkg")]
pub mod rpkg;
pub mod convert;
pub mod pack;
pub mod enums;
pub mod mipblock;
pub mod atlas;
#[cfg(feature = "image")]
pub mod image;
pub mod box_reflection;

#[non_exhaustive]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GlacierGame {
    HM2016,
    HM2,
    HM3,
    KNT,
}

impl FromStr for GlacierGame {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "1" | "H1" | "HM1" | "HM2016" | "H2016" => Ok(GlacierGame::HM2016),
            "2" | "H2" | "HM2" | "HM2018" | "H2018" => Ok(GlacierGame::HM2),
            "3" | "H3" | "HM3" | "HM2020" | "H2020" => Ok(GlacierGame::HM3),
            "007" | "KNT" | "BOND" | "FIRSTLIGHT" | "B2026" => Ok(GlacierGame::KNT),
            _ => Err(format!("Invalid value for GlacierGame: {s}")),
        }
    }
}

pub trait Version {
    fn get_version() -> GlacierGame;
}
