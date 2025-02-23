use std::fmt::{Debug};
use binrw::{binrw, binwrite, BinRead};
use std::io::{Read, Seek, Write};
use std::ops::Index;
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian};

use crate::utils::math::Vector3;

// possible cloth ids
// 0    0x00    00000000
// 1    0x01    00000001
// 2    0x02    00000010
// 3    0x03    00000011
// 4    0x04    00000100
// 5    0x05    00000101
// 6    0x06    00000110
// 7    0x07    00000111
// 129  0x81    10000001
// 130  0x82    10000010
// 131  0x83    10000011
// 132  0x84    10000100
// 133  0x85    10000101
// 193  0xC1    11000001
// 194  0xC2    11000010
// 195  0xC3    11000011
// 196  0xC4    11000100


#[binread]
#[derive(BinWrite, Debug, PartialEq, Clone)]
#[br(import{
cloth_id: u8,
num_vertices: u32})]
//sim and cloth prims
pub enum ClothSimMesh {

    #[br(pre_assert(cloth_id & 0x80 == 0x80))] //disabled, is unstable
    Simulation(ClothSimPack),

    #[br(pre_assert(cloth_id & 0x80 != 0x80))]
    Skinned(
        #[br(count = num_vertices)]
        Vec<ClothSkinning>
    ),
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Clone)]
pub struct ClothSkinning
{
    pub indices: [u16; 4],
    pub weights: [u16; 4],
    pub simulation_bias: u16,
    pub simulation_weight: u16,
}


#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct ClothSimPack {
    #[br(temp)]
    pub header: PackHeader,

    #[br(if(header.properties_size > 0), args{total_size: header.properties_size})]
    pub simulation_properties: Option<SimulationProperties>,

    #[br(count = (header.grid_size as usize / size_of::<GridPoint>()) * 2)]
    pub grid_points: Vec<GridPoint>,

    #[br(count = header.unknown_count)]
    pub unknown: Vec<UnkStruct>,
}

#[binrw]
#[derive(Debug, PartialEq, Clone)]
pub struct UnkStruct {
    pub unk1: u16, //m_nAnchorDist?
    pub unk2: u16, //m_nParticleIndex
}


impl BinWrite for ClothSimPack {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        let properties_size = match &self.simulation_properties{
            None => {0}
            Some(properties) => {
                if properties.constrain_properties.skinning_constrain_scale_v.is_some() { 0x94 } else { 0x74 }
            }
        };
        let unknown_count = self.unknown.len() as u16;
        let grid_size = (self.grid_points.len() * size_of::<GridPoint>()) as u32 / 2;
        let header = PackHeader{
            data_size: size_of::<PackHeader>() as u32 + properties_size as u32 + (unknown_count * 4) as u32 + grid_size,
            properties_size,
            unknown_count,
            grid_size,
        };

        header.write_options(writer, endian, args)?;
        self.simulation_properties.write_options(writer, endian, args)?;
        self.grid_points.write_options(writer, endian, args)?;
        self.unknown.write_options(writer, endian, args)?;
        Ok(())
    }
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Default, Clone)]
pub struct PackHeader
{
    pub(crate) data_size: u32,
    pub(crate) properties_size: u16,
    pub(crate) unknown_count: u16,
    pub(crate) grid_size: u32,
}

#[binrw]
#[derive(Clone, Debug, PartialEq)]
#[br(import{
total_size: u16})]
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

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_per_vertex_stiffness: bool,

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_per_vertex_damping: bool,

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_per_vertex_skinning: bool,

    #[br(temp)]
    #[bw(calc = 0)]
    pad: u8,

    #[br(args{new_format: total_size > 0x74})]
    constrain_properties: ConstrainProperties, //if total size is 148 (0x94), this changes

    #[br(if(total_size > 0x74), count = 6)]
    unknown: Vec<u32>
    //6x u32 should be added here when the size is 148 (0x94)
}

#[binrw]
#[derive(Clone, Debug, PartialEq)]
#[br(import{new_format: bool})]
struct ConstrainProperties
{
    shear_stiffness: f32,
    bend_stiffness: f32,
    bend_curvature: f32,

    #[br(if(!new_format))]
    skinning_constrain_scale: Option<f32>, //This has become a vec3 in H2

    #[br(if(new_format))]
    skinning_constrain_scale_v: Option<Vector3>,

    max_motion: f32,
    anchor_stretch: f32,
    lra_stretch: f32,
    parent_dist_stretch: f32,
    bend_constrain_type: ClothBendConstrainType,
    stretch_constrain_type: ClothStretchConstrainType,
    num_constrain_iterations: u32,
    anchor_stretch_direction: [Neighbor; 4],
    num_anchor_stretch_direction: u32,

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_parent_dist_constrains: bool,

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_sphere_skinning_constrains: bool,

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_pos_normal_constrains: bool,

    #[br(map = |x: u8| x > 0)]
    #[bw(map = |x| if *x {1} else {0})]
    use_neg_normal_constrains: bool,
}

#[binrw]
#[derive(Clone, Debug, PartialEq)]
#[repr(u32)]
#[brw(repr = u32)]
enum Neighbor { Down, DownRight, Right, UpRight, Up, UpLeft, Left, DownLeft }


#[derive(Debug, PartialEq, Clone)]
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

#[binrw]
#[repr(u32)]
#[derive(Clone, Debug, PartialEq)]
#[brw(repr = u32)]
enum ClothBendConstrainType
{
    Stick = 0,
    Triangle = 1,
}

#[binrw]
#[repr(u32)]
#[derive(Clone, Debug, PartialEq)]
#[brw(repr = u32)]
enum ClothStretchConstrainType
{
    Anchor = 0,
    Lra = 1,
    None = 2,
}