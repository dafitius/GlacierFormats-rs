use std::str::FromStr;

pub mod texture_map;

pub mod atlas;
pub mod box_reflection;
pub mod convert;
pub mod enums;
#[cfg(feature = "image")]
pub mod image;
pub mod mipblock;
pub mod pack;
#[cfg(feature = "rpkg")]
pub mod rpkg;

#[non_exhaustive]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GlacierGame {
    HM2016,
    HM2,
    HM3,
    KNT,
}

impl GlacierGame{
    pub(crate) fn supports_lz4_compression(&self) -> bool{
        match self{
            GlacierGame::HM2016 | GlacierGame::HM2 => false,
            GlacierGame::HM3 | GlacierGame::KNT => true
        }
    }
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
