use std::fmt::{Debug};
use binrw::BinRead;
use std::io::{Read, Seek, Write};
use std::ops::Index;
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian};

#[cfg(feature = "serde")]
use serde::{Serialize};
use crate::math::Vector3;


#[binread]
#[derive(BinWrite, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[br(import{
cloth_id: u8,
num_vertices: u32})]
pub enum ClothData {
    #[br(pre_assert(cloth_id & 0x80 != 0x80))]
    Skinned(
        #[br(count = num_vertices)]
        Vec<ClothSkinning>
    ),

    #[br(pre_assert(cloth_id & 0x80 == 0x80))]
    Packed(ClothPack),
}

#[binread]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct ClothPack {
    #[br(temp)]
    pub header: PackHeader,

    #[br(count = header.simulation_size)]
    pub simulation_properties: Vec<u8>,

    #[br(count = header.grid_size / 0x10)]
    pub grid_points: Vec<GridPoint>,

    #[br(count = header.properties_size)]
    pub unknown: Vec<u32>,
}

impl BinWrite for ClothPack {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        todo!()
    }
}

#[derive(BinRead, BinWrite, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct ClothSkinning
{
    pub indices: [u16; 4],
    pub weights: [u16; 4],
    pub simulation_bias: u16,
    pub simulation_weight: u16,
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Default, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct PackHeader
{
    pub(crate) data_size: u32,
    pub(crate) simulation_size: u16,
    pub(crate) properties_size: u16,
    pub(crate) grid_size: u32,
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct GridPoint
{
    pub down: Option<u16>,
    pub down_right: Option<u16>,
    pub right: Option<u16>,
    pub up_right: Option<u16>,
    pub up: Option<u16>,
    pub up_left: Option<u16>,
    pub left: Option<u16>,
    pub down_left: Option<u16>,
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[repr(u32)]
enum Neighbor { Down, DownRight, Right, UpRight, Up, UpLeft, Left, DownLeft }

impl Index<Neighbor> for GridPoint {
    type Output = Option<u16>;

    fn index(&self, index: Neighbor) -> &Self::Output {
        match index {
            Neighbor::Down => &self.down,
            Neighbor::DownRight => &self.down_right,
            Neighbor::Right => &self.right,
            Neighbor::UpRight => &self.up_right,
            Neighbor::Up => &self.up,
            Neighbor::UpLeft => &self.up_left,
            Neighbor::Left => &self.left,
            Neighbor::DownLeft => &self.down_left,
        }
    }
}

impl BinRead for GridPoint {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(reader: &mut R, endian: Endian, args: Self::Args<'_>) -> BinResult<Self> {
        fn read_optional_u16(reader: &mut (impl Read + std::io::Seek), endian: Endian) -> BinResult<Option<u16>> {
            let value = u16::read_options(reader, endian, ())?;
            Ok(if value == u16::MAX { None } else { Some(value) })
        }
        Ok(GridPoint {
            down: read_optional_u16(reader, endian)?,
            down_right: read_optional_u16(reader, endian)?,
            right: read_optional_u16(reader, endian)?,
            up_right: read_optional_u16(reader, endian)?,
            up: read_optional_u16(reader, endian)?,
            up_left: read_optional_u16(reader, endian)?,
            left: read_optional_u16(reader, endian)?,
            down_left: read_optional_u16(reader, endian)?,
        })
    }
}

impl BinWrite for GridPoint {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        writer.write_type(&self.down.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.down_right.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.right.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.up_right.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.up.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.up_left.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.left.unwrap_or(u16::MAX), endian)?;
        writer.write_type(&self.down_left.unwrap_or(u16::MAX), endian)?;
        Ok(())
    }
}

pub struct SimulationProperties
{
    root_bone: u32,
    frequency: f32,
    collision_offset: f32,
    damping: f32,
    gravity: Vector3,
    drag_constant: f32,
    wind_constant: f32,
    zbias: f32,
    collision_groups: u32,
    use_per_vertex_stiffness: bool,
    use_per_vertex_damping: bool,
    use_per_vertex_skinning: bool,
    pad: bool,
    constrain_properties: ConstrainProperties,
}

struct ConstrainProperties
{
    shear_stiffness: f32,
    bend_stiffness: f32,
    bend_curvature: f32,
    skinning_constrain_scale: f32,
    max_motion: f32,
    anchor_stretch: f32,
    lra_stretch: f32,
    parent_dist_stretch: f32,
    bend_constrain_type: ClothBendConstrainType,
    stretch_constrain_type: ClothStretchConstrainType,
    num_constrain_iterations: u32,
    anchor_stretch_direction: [Neighbor; 4],
    num_anchor_stretch_direction: u32,
    use_parent_dist_constrains: bool,
    use_sphere_skinning_constrains: bool,
    use_pos_normal_constrains: bool,
    use_neg_normal_constrains: bool,
}

#[repr(u32)]
enum ClothBendConstrainType
{
    Stick = 0,
    Triangle = 1,
}

#[repr(u32)]
enum ClothStretchConstrainType
{
    Anchor = 0,
    Lra = 1,
    None = 2,
}