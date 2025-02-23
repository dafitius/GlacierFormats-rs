use binrw::{BinRead, binrw, BinWrite};
use bitfield_struct::bitfield;
use crate::render_primitive::PrimHeader;
use crate::utils::math::{BoundingBox, Vector3};

#[binrw]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone, Copy)]
#[bw(import(bb: BoundingBox<Vector3>))]
#[br(assert(sub_type != PrimObjectSubtype::Speedtree, "Speedtree is not supported yet"))]
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
#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
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
#[bitfield(u8)]
#[derive(BinRead, BinWrite, PartialEq)]
pub struct ObjectPropertyFlags
{
    pub x_axis_locked: bool,
    pub y_axis_locked: bool,
    pub z_axis_locked: bool,
    pub has_highres_positions: bool,
    __: bool,
    pub has_constant_color: bool,
    pub is_no_physics_prop: bool,
    __: bool,
}