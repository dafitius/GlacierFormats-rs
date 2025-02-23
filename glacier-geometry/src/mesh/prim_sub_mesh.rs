
use std::io::{Seek, SeekFrom, Write};
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};
use crate::cloth::cloth::ClothSimMesh;
use crate::collision::collision::{BoxColi, Collision};
use crate::model::prim_mesh::PrimMesh;
use crate::model::prim_object::{ObjectPropertyFlags, PrimObject};
use crate::render_primitive::{align_writer, PrimPropertyFlags};
use crate::utils::buffer;
use crate::utils::buffer::{IndexBuffer, VertexBuffers};
use crate::utils::math::{BoundingBox, Vector2, Vector3, Vector4};


#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import{
global_properties: PrimPropertyFlags,
mesh_properties: ObjectPropertyFlags,
cloth_id: u8
})]
pub struct PrimSubMesh
{
    pub prim_object: PrimObject,


    pub num_vertices: u32,

    #[br(temp)]
    vertices_offset: u32,

    #[br(temp)]
    num_indices: u32,

    #[br(temp)]
    num_cracked_indices: u32,
    #[br(
    parse_with = FilePtr32::parse,
    args{inner: binrw::args ! { count: (num_indices + num_cracked_indices) as usize}}
    )]
    pub(crate) indices: IndexBuffer,

    #[br(
    parse_with = FilePtr32::parse,
    args{inner: binrw::args!{global_properties: global_properties}}
    )]
    pub collision: Collision,

    #[br(temp)]
    pub cloth_offset : u32,

    #[br(pad_after(3))]
    pub num_uv_channels: u8,

    #[br(
    seek_before = SeekFrom::Start(vertices_offset as u64),
    restore_position,
    parse_with = buffer::parse_vertices,
    args(
    num_vertices,
    mesh_properties.has_highres_positions(),
    global_properties.is_weighted_object(),
    num_uv_channels,
    prim_object.properties.has_constant_color(),
    mesh_properties.has_constant_color())
    )]
    pub(crate) buffers: VertexBuffers,


    #[br(
    if(cloth_offset != 0),
    seek_before = SeekFrom::Start(cloth_offset as u64),
    restore_position,
    args{
    cloth_id: cloth_id,
    num_vertices: num_vertices,
    }
    )]
    pub cloth_data: Option<ClothSimMesh>,
}


impl BinWrite for PrimSubMesh {
    type Args<'a> = (&'a PrimMesh, &'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        let mesh = args.0;
        let property_flags = args.1;

        let mut collision_offset = 0;
        if property_flags.is_linked_object() {
            collision_offset = writer.stream_position()? as u32;
            writer.write_type(&self.collision, endian)?;
            align_writer(writer, 16)?;
        }

        let index_offset = writer.stream_position()? as u32;
        for index in self.indices.iter() {
            writer.write_type(index, endian)?;
        }
        align_writer(writer, 16)?;

        let vertex_offset = writer.stream_position()? as u32;
        VertexBuffers::write_options(&self.buffers, writer, endian, ())?;
        align_writer(writer, 16)?;

        if !property_flags.is_linked_object() {
            collision_offset = writer.stream_position()? as u32;
            writer.write_type(&self.collision, endian)?;
            align_writer(writer, 16)?;
        }

        let cloth_offset = writer.stream_position()? as u32;
        if let Some(cloth_data) = &self.cloth_data{
            writer.write_type(cloth_data, endian)?;
            align_writer(writer, 16)?;
        }

        let header_offset = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_object, writer, endian, (mesh.calc_bb(),))?;
        writer.write_type(&self.num_vertices, endian)?;
        writer.write_type(&vertex_offset, endian)?;
        writer.write_type(&(self.indices.len() as u32), endian)?;
        writer.write_type(&0u32, endian)?; //todo: figure out when this should be more
        writer.write_type(&index_offset, endian)?;

        writer.write_type(&collision_offset, endian)?;

        writer.write_type(&(if self.cloth_data.is_some() {cloth_offset} else {0}), endian)?; //cloth
        writer.write_type(&(self.num_uv_channels as u32), endian)?;
        align_writer(writer, 16)?;

        *args.2 = writer.stream_position()? as u32;

        writer.write_type(&header_offset, endian)?;
        writer.write_type(&0u32, endian)?; //todo: change this to use align_writer
        writer.write_type(&0u64, endian)?;

        Ok(())
    }
}
