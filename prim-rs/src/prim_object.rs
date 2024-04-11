use binrw::{BinRead, binrw, BinWrite};
use modular_bitfield::bitfield;
use crate::math::{BoundingBox, Vector3};
use crate::render_primitive::PrimHeader;

#[cfg(feature = "serde")]
use serde::{Serialize, Serializer};
#[cfg(feature = "serde")]
use serde::ser::SerializeStruct;

#[binrw]
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[bw(import(bb: BoundingBox<Vector3>))]
pub struct PrimObject
{
    pub prims: PrimHeader,
    pub sub_type: PrimObjectSubtype,
    pub properties: ObjectPropertyFlags,
    #[brw(pad_after(1))]
    pub lod_mask: u8,
    pub z_bias: u8,
    pub z_offset: u8,
    pub material_id: u16,
    pub wire_color: u32,
    pub constant_vertex_color: u32,

    #[br(temp)]
    #[bw(calc = bb.min)]
    pub min: Vector3,

    #[br(temp)]
    #[bw(calc = bb.max)]
    pub max: Vector3,
}

#[allow(dead_code)]
#[derive(BinRead, BinWrite, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[brw(little, repr = u8)]
pub enum PrimObjectSubtype
{
    Standard = 0,
    Linked = 1,
    Weighted = 2,
    Standarduv2 = 3,
    Standarduv3 = 4,
    Standarduv4 = 5,
    Speedtree = 6,
}

#[allow(redundant_semicolons)]
#[bitfield(bytes = 1)]
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq)]
pub struct ObjectPropertyFlags
{
    pub x_axis_locked: bool,
    pub y_axis_locked: bool,
    pub z_axis_locked: bool,
    pub has_highres_positions: bool,
    #[skip]
    __: bool,
    pub has_constant_color: bool,
    pub is_no_physics_prop: bool,
    #[skip]
    __: bool,
}

#[cfg(feature = "serde")]
impl Serialize for ObjectPropertyFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut s = serializer.serialize_struct("PrimPropertyFlags", 6)?;
        s.serialize_field("x_axis_locked", &self.x_axis_locked())?;
        s.serialize_field("y_axis_locked", &self.y_axis_locked())?;
        s.serialize_field("z_axis_locked", &self.z_axis_locked())?;
        s.serialize_field("has_highres_positions", &self.has_highres_positions())?;
        s.serialize_field("has_constant_color", &self.has_constant_color())?;
        s.serialize_field("is_no_physics_prop", &self.is_no_physics_prop())?;
        s.end()
    }
}
