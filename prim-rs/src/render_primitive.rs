
use std::{fs, io};
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path};
use binrw::{BinRead, binread, BinReaderExt, BinResult, BinWrite, BinWriterExt, Endian, FilePtr64};
use binrw::io::SeekFrom;
use bitfield_struct::bitfield;
use crate::math::{BoundingBox, Vector3};
use crate::prim_mesh_weighted::PrimMeshWeighted;

use crate::prim_mesh::PrimMesh;

#[binread]
#[derive(Debug, PartialEq, Clone)]
#[brw(little)]
pub struct RenderPrimitive {
    #[br(parse_with = FilePtr64::parse)]
    pub data: PrimObjectHeader,
}

impl RenderPrimitive {
    pub fn parse(path: &Path) -> BinResult<RenderPrimitive> {
        let mut reader = Cursor::new(fs::read(path).unwrap());
        let prim: RenderPrimitive = reader.read_ne()?;
        Ok(prim)
    }

    pub fn parse_bytes<A : Read + Seek>(data: &mut A) -> BinResult<RenderPrimitive> {
        let prim = RenderPrimitive::read_le_args(data, ())?;
        Ok(prim)
    }


    pub fn write(&self, path: &Path) -> Result<(), binrw::Error> {
        let mut writer = Cursor::new(Vec::new());
        self.write_options(&mut writer, Endian::Little, ())?;
        fs::write(path, writer.into_inner())?;
        Ok(())
    }
}

impl BinWrite for RenderPrimitive {
    type Args<'a> = ();
    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, _args: Self::Args<'_>) -> BinResult<()> {
        let mut header_pointer: u64 = 0;
        let padd: u64 = 0;
        u64::write_options(&header_pointer, writer, endian, ())?;
        u64::write_options(&padd, writer, endian, ())?;


        PrimObjectHeader::write_options(&self.data, writer, endian, &mut header_pointer)?;

        writer.seek(SeekFrom::Start(0))?;
        u64::write_options(&header_pointer, writer, endian, ())?;

        Ok(())
    }
}

#[binread]
#[allow(dead_code, unused_variables)]
#[derive(Debug, PartialEq, Clone)]
pub struct PrimObjectHeader
{
    pub prims: PrimHeader,

    pub property_flags: PrimPropertyFlags,

    #[br(map = |x: u32| if x != 0xFFFFFFFF {Some(x)} else {None})]
    bone_rig_resource_index: Option<u32>,

    #[br(temp)]
    pub num_objects: u32,

    #[br(
    parse_with = parse_objects,
    args(num_objects, property_flags,),
    )]
    pub objects: Vec<MeshObject>,

    #[br(temp)]
    pub min: Vector3,

    #[br(temp)]
    pub max: Vector3,
}

impl BinWrite for PrimObjectHeader {
    type Args<'a> = &'a mut u64;
    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let mut obj_offsets = (0..self.objects.len()).map(|_| 0).collect::<Vec<u32>>();

        for (i, object) in self.objects.iter().enumerate() {
            MeshObject::write_options(object, writer, endian, (&self.property_flags, &mut obj_offsets[i]))?;
        }


        let object_table_start_pos = writer.stream_position()?;
        for offset in obj_offsets {
            u32::write_options(&offset, writer, endian, ())?;
        }
        align_writer(writer, 16)?;

        let header_start_pos = writer.stream_position()?;
        *args = header_start_pos;
        writer.write_type(&self.prims, endian)?;
        writer.write_type(&self.property_flags, endian)?;
        writer.write_type(&self.bone_rig_resource_index.unwrap_or(0xFFFFFFFF), endian)?;
        writer.write_type(&u32::try_from(self.objects.len()).unwrap_or(0), endian)?;
        writer.write_type(&(object_table_start_pos as u32), endian)?;

        let bb : BoundingBox<Vector3> = self.objects.iter().map(|o| match o{
            MeshObject::Normal(o) => {o.sub_mesh.calc_bb()}
            MeshObject::Weighted(o) => {o.prim_mesh.sub_mesh.calc_bb()}
            MeshObject::Linked(o) => {o.prim_mesh.sub_mesh.calc_bb()}
        }).sum();

        writer.write_type(&bb.min, endian)?;
        writer.write_type(&bb.max, endian)?;
        align_writer(writer, 8)?;

        Ok(())
    }
}

#[derive(BinRead, Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags))]
pub enum MeshObject {
    #[br(pre_assert(!global_properties.is_weighted_object() && !global_properties.is_linked_object()))]
    Normal(
        #[br(args(global_properties))]
        PrimMesh
    ),
    #[br(pre_assert(global_properties.is_weighted_object()))]
    Weighted(
        #[br(args(global_properties))]
        PrimMeshWeighted
    ),
    #[br(pre_assert(global_properties.is_linked_object()))]
    Linked(
        #[br(args(global_properties))]
        PrimMeshWeighted
    ),
}

impl BinWrite for MeshObject {
    type Args<'a> = (&'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        match self {
            MeshObject::Normal(obj) => { PrimMesh::write_options(obj, writer, endian, args)? }
            MeshObject::Weighted(obj) => { PrimMeshWeighted::write_options(obj, writer, endian, args)? }
            MeshObject::Linked(obj) => { PrimMeshWeighted::write_options(obj, writer, endian, args)? }
        };
        Ok(())
    }
}

#[allow(redundant_semicolons)]
#[bitfield(u32)]
#[derive(Eq, Hash, PartialEq)]
#[derive(BinRead, BinWrite)]
pub struct PrimPropertyFlags
{
    pub has_bones: bool,
    pub has_frames: bool,
    pub is_linked_object: bool,
    pub is_weighted_object: bool,
    #[bits(4)]
    __: u8,
    pub use_bounds: bool,
    pub has_highres_positions: bool,
    #[bits(22)]
    __: usize,
}

#[allow(dead_code)]
#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
pub struct PrimHeader
{
    #[brw(pad_before(2))]
    pub type_: PrimType,
}

#[allow(dead_code)]
#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
#[brw(little, repr = u16)]
pub enum PrimType
{
    None = 0,
    ObjectHeader = 1,
    Mesh = 2,
    Shape = 5,
}



#[binrw::parser(reader, endian)]
fn parse_objects(object_count: u32, global_properties: PrimPropertyFlags) -> BinResult<Vec<MeshObject>> {
    let table_offset = u32::read_options(reader, endian, ())?;
    let saved_pos = reader.stream_position()?;
    reader.seek(SeekFrom::Start(table_offset as u64))?;

    let offset_table = (0..object_count).map(|_| {
        u32::read_options(reader, endian, ())
    });

    let mut objects = vec![];
    for offset in offset_table.flatten().collect::<Vec<_>>() {
        reader.seek(SeekFrom::Start(offset as u64))?;
        objects.push(MeshObject::read_options(reader, endian, (global_properties, ))?);
    }

    reader.seek(SeekFrom::Start(saved_pos))?;
    Ok(objects)
}


pub fn align_writer<W: Write + Seek>(writer: &mut W, num: usize) -> Result<(), io::Error> {
    let padding = (num - (writer.stream_position()? as usize % num)) % num;
    writer.write_all(vec![0; padding].as_slice())?;
    Ok(())
}