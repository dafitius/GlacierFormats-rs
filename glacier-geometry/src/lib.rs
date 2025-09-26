pub mod cloth;
pub mod collision;
pub mod mesh;
pub mod model;
// pub mod rig;
pub mod utils;

pub mod rig;

#[cfg(feature = "rpkg")]
pub mod rpkg;
pub mod render_primitive;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum WoaVersion {
    HM2016,
    HM2,
    HM3,
}