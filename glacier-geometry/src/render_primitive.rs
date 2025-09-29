
use crate::utils::io::align_writer;
use std::fs;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path};
use binrw::{BinRead, binread, BinReaderExt, BinResult, BinWrite, BinWriterExt, Endian, FilePtr64};
use binrw::io::SeekFrom;
use bitfield_struct::bitfield;
use crate::model::prim_mesh::PrimMesh;
use crate::model::prim_mesh_weighted::PrimMeshWeighted;
use crate::utils::math::{BoundingBox, Color, Vector2, Vector3, Vector4};
use crate::model::prim_mesh_linked::PrimMeshLinked;
use crate::utils::buffer::{IndexBuffer, Vertex, VertexWeights};
use crate::WoaVersion;

#[binread]
#[derive(Debug, PartialEq, Clone)]
#[brw(little)]
#[br(import(woa_version: WoaVersion))]
pub struct RenderPrimitive {
    #[br(parse_with = FilePtr64::parse)]
    #[br(args{ inner: (woa_version,)})]
    data: PrimObjectHeader,
}

pub enum LodLevel{
    LEVEL1,
    LEVEL2,
    LEVEL3,
    LEVEL4,
    LEVEL5,
    LEVEL6,
    LEVEL7,
    LEVEL8,
}

impl RenderPrimitive {
    pub fn parse<P: AsRef<Path>>(path: P, woa_version: WoaVersion) -> BinResult<RenderPrimitive> {
        let mut reader = Cursor::new(fs::read(path)?);
        let prim: RenderPrimitive = reader.read_le_args((woa_version,))?;
        Ok(prim)
    }

    pub fn parse_bytes<A : Read + Seek>(data: &mut A, woa_version: WoaVersion) -> BinResult<RenderPrimitive> {
        let prim : RenderPrimitive= data.read_le_args((woa_version,))?;
        Ok(prim)
    }

    pub fn write(&self, path: &Path, woa_version: WoaVersion) -> Result<(), binrw::Error> {
        let mut writer = Cursor::new(Vec::new());
        self.write_options(&mut writer, Endian::Little, &woa_version)?;
        fs::write(path, writer.into_inner())?;
        Ok(())
    }

    pub fn iter_primitives(&self) -> impl Iterator<Item = &MeshObject> {
        self.data.objects.iter()
    }

    pub fn iter_primitive_of_lod(&self, lod: LodLevel) -> impl Iterator<Item = &MeshObject> {
        let lod_mask = match lod {
            LodLevel::LEVEL8 => {0b1}
            LodLevel::LEVEL7 => {0b10}
            LodLevel::LEVEL6 => {0b100}
            LodLevel::LEVEL5 => {0b1000}
            LodLevel::LEVEL4 => {0b10000}
            LodLevel::LEVEL3 => {0b100000}
            LodLevel::LEVEL2 => {0b1000000}
            LodLevel::LEVEL1 => {0b10000000}
        };
        self.data.objects.iter().filter(move |&obj| obj.prim_mesh().prim_object.lod_mask & lod_mask == lod_mask)
    }

    pub fn primitives_count(&self) -> usize {
        self.data.objects.len()
    }

    pub fn flags(&self) -> PrimPropertyFlags {
        self.data.property_flags
    }
}

impl BinWrite for RenderPrimitive {
    type Args<'a> = &'a WoaVersion;
    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let mut header_pointer: u64 = 0;
        let padd: u64 = 0;
        u64::write_options(&header_pointer, writer, endian, ())?;
        u64::write_options(&padd, writer, endian, ())?;

        PrimObjectHeader::write_options(&self.data, writer, endian, (args, &mut header_pointer))?;

        writer.seek(SeekFrom::Start(0))?;
        u64::write_options(&header_pointer, writer, endian, ())?;

        Ok(())
    }
}

#[binread]
#[allow(dead_code, unused_variables)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(woa_version: WoaVersion))]
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
    args(num_objects, property_flags, woa_version),
    )]
    pub objects: Vec<MeshObject>,

    #[br(temp)]
    pub _min: Vector3,

    #[br(temp)]
    pub _max: Vector3,
}

impl BinWrite for PrimObjectHeader {
    type Args<'a> = (&'a WoaVersion, &'a mut u64);
    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let mut obj_offsets = (0..self.objects.len()).map(|_| 0).collect::<Vec<u32>>();

        for (i, object) in self.objects.iter().enumerate() {
            MeshObject::write_options(object, writer, endian, (args.0, &self.property_flags, &mut obj_offsets[i]))?;
        }


        let object_table_start_pos = writer.stream_position()?;
        for offset in obj_offsets {
            u32::write_options(&offset, writer, endian, ())?;
        }
        align_writer(writer, 16)?;

        let header_start_pos = writer.stream_position()?;
        *args.1 = header_start_pos;
        writer.write_type(&self.prims, endian)?;
        writer.write_type(&self.property_flags, endian)?;
        writer.write_type(&self.bone_rig_resource_index.unwrap_or(0xFFFFFFFF), endian)?;
        writer.write_type(&u32::try_from(self.objects.len()).unwrap_or(0), endian)?;
        writer.write_type(&(object_table_start_pos as u32), endian)?;

        let bb : BoundingBox<Vector3> = self.objects.iter().map(|o| o.prim_mesh().calc_bb()).sum();

        writer.write_type(&bb.min, endian)?;
        writer.write_type(&bb.max, endian)?;
        align_writer(writer, 8)?;

        Ok(())
    }
}

#[derive(BinRead, Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags, woa_version: WoaVersion))]
pub enum MeshObject {
    #[br(pre_assert(!global_properties.is_weighted_object() && !global_properties.is_linked_object()))]
    Normal(
        #[br(args(woa_version, global_properties))]
        PrimMesh
    ),
    #[br(pre_assert(global_properties.is_weighted_object()))]
    Weighted(
        #[br(args(woa_version, global_properties))]
        PrimMeshWeighted
    ),
    #[br(pre_assert(global_properties.is_linked_object()))]
    Linked(
        #[br(args(woa_version, global_properties))]
        PrimMeshLinked
    )
}

impl MeshObject {
    pub fn prim_mesh(&self) -> &PrimMesh {
        match self {
            MeshObject::Normal(prim_mesh) => prim_mesh,
            MeshObject::Weighted(prim_mesh_weighted) => &prim_mesh_weighted.prim_mesh,
            MeshObject::Linked(prim_mesh_weighted) => &prim_mesh_weighted.prim_mesh,
        }
    }

    pub fn get_indices(&self) -> &IndexBuffer {
        &self.prim_mesh().sub_mesh.indices
    }

    pub fn get_vertices(&self) -> Vec<Vertex> {
       self.prim_mesh().get_vertices()
    }

    pub fn get_positions(&self) -> Vec<Vector4> {
        self.prim_mesh().get_positions()
    }

    pub fn get_weights(&self) -> Option<Vec<VertexWeights>> {
        self.prim_mesh().get_weights()
    }

    pub fn get_normals(&self) -> Vec<Vector4> {
        self.prim_mesh().get_normals()
    }

    pub fn get_tangents(&self) -> Vec<Vector4> {
        self.prim_mesh().get_tangents()
    }

    pub fn get_bitangents(&self) -> Vec<Vector4> {
        self.prim_mesh().get_bitangents()
    }

    pub fn get_tex_coords(&self) -> Vec<Vec<Vector2>> {
        self.prim_mesh().get_tex_coords()
    }

    pub fn get_colors(&self) -> Option<Vec<Color>> {
        self.prim_mesh().get_colors()
    }
}

impl BinWrite for MeshObject {
    type Args<'a> = (&'a WoaVersion, &'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        match self {
            MeshObject::Normal(obj) => { PrimMesh::write_options(obj, writer, endian, args)? }
            MeshObject::Weighted(obj) => { PrimMeshWeighted::write_options(obj, writer, endian, args)? }
            MeshObject::Linked(obj) => { PrimMeshLinked::write_options(obj, writer, endian, args)? }
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
fn parse_objects(object_count: u32, global_properties: PrimPropertyFlags, woa_version: WoaVersion) -> BinResult<Vec<MeshObject>> {
    let table_offset = u32::read_options(reader, endian, ())?;
    let saved_pos = reader.stream_position()?;
    reader.seek(SeekFrom::Start(table_offset as u64))?;

    let offset_table = (0..object_count).map(|_| {
        u32::read_options(reader, endian, ())
    });

    let mut objects = vec![];
    for offset in offset_table.flatten().collect::<Vec<_>>() {
        reader.seek(SeekFrom::Start(offset as u64))?;
        objects.push(MeshObject::read_options(reader, endian, (global_properties, woa_version))?);
    }

    reader.seek(SeekFrom::Start(saved_pos))?;
    Ok(objects)
}