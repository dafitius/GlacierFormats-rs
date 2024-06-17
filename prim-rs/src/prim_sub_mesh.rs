use crate::cloth::ClothData;
use crate::collision::BoxColi;
use std::io::{Seek, SeekFrom, Write};
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};
use crate::math::{BoundingBox, Vector2, Vector3, Vector4};
use crate::prim_mesh::PrimMesh;
use crate::prim_object::{ObjectPropertyFlags, PrimObject};
use crate::render_primitive::{align_writer, PrimPropertyFlags};
use crate::buffer;
use crate::buffer::{IndexBuffer, VertexBuffers};


#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
#[br(import{
global_properties: PrimPropertyFlags,
mesh_properties: ObjectPropertyFlags,
cloth_id: u8,
pos_scale: Vector4,
pos_bias: Vector4,
tex_scale_bias: Vector4,
})]
pub struct PrimSubMesh
{
    pub prim_object: PrimObject,

    #[br(temp)]
    num_vertices: u32,

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
    pub indices: IndexBuffer,

    #[br(
    parse_with = FilePtr32::parse,
    )]
    pub collision: BoxColi,

    pub cloth_offset : u32,

    #[br(temp, pad_after(3))]
    num_uv_channels: u8,

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
    mesh_properties.has_constant_color(),
    pos_scale,
    pos_bias,
    tex_scale_bias,
    )
    )]
    pub buffers: VertexBuffers,


    #[br(
    if(cloth_offset != 0),
    seek_before = SeekFrom::Start(cloth_offset as u64),
    restore_position,
    args{
    cloth_id: cloth_id,
    num_vertices: buffers.num_vertices(),
    }
    )]
    pub cloth_data: Option<ClothData>,
}

impl PrimSubMesh {
    pub fn calc_bb(&self) -> BoundingBox<Vector3> {
        let mut min_bb = Vector3 { x: f32::INFINITY, y: f32::INFINITY, z: f32::INFINITY };
        let mut max_bb = Vector3 { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY, z: f32::NEG_INFINITY };

        for pos in self.buffers.position.iter() {
            min_bb.x = min_bb.x.min(pos.x);
            max_bb.x = max_bb.x.max(pos.x);

            min_bb.y = min_bb.y.min(pos.y);
            max_bb.y = max_bb.y.max(pos.y);

            min_bb.z = min_bb.z.min(pos.z);
            max_bb.z = max_bb.z.max(pos.z);
        };
        BoundingBox { min: min_bb, max: max_bb }
    }

    pub fn calc_uv_bb(&self) -> BoundingBox<Vector2> {
        let mut min_bb = Vector2 { x: f32::INFINITY, y: f32::INFINITY };
        let mut max_bb = Vector2 { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY };

        for layer in self.buffers.main.iter() {
            for pos in layer.uvs.iter() {
                min_bb.x = min_bb.x.min(pos.x);
                max_bb.x = max_bb.x.max(pos.x);

                min_bb.y = min_bb.y.min(pos.y);
                max_bb.y = max_bb.y.max(pos.y);
            };
        }

        BoundingBox {
            min: min_bb,
            max: max_bb,
        }
    }
}

impl BinWrite for PrimSubMesh {
    type Args<'a> = (&'a PrimMesh, &'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let index_offset = writer.stream_position()? as u32;
        for index in self.indices.iter() {
            writer.write_type(index, endian)?;
        }
        align_writer(writer, 16)?;

        let vertex_offset = writer.stream_position()? as u32;
        VertexBuffers::write_options(&self.buffers, writer, endian, (args.0, &self.prim_object.properties, args.1))?;
        align_writer(writer, 16)?;

        let collision_offset = writer.stream_position()? as u32;
        writer.write_type(&self.collision, endian)?;
        align_writer(writer, 16)?;

        let cloth_offset = writer.stream_position()? as u32;
        if let Some(cloth_data) = &self.cloth_data{
            writer.write_type(cloth_data, endian)?;
            align_writer(writer, 16)?;
        }

        let header_offset = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_object, writer, endian, (self.calc_bb(),))?;
        writer.write_type(&self.buffers.num_vertices(), endian)?;
        writer.write_type(&vertex_offset, endian)?;
        writer.write_type(&(self.indices.len() as u32), endian)?;
        writer.write_type(&0u32, endian)?; //todo: figure out when this should be more
        writer.write_type(&index_offset, endian)?;
        writer.write_type(&collision_offset, endian)?;
        writer.write_type(&(if self.cloth_data.is_some() {cloth_offset} else {0}), endian)?; //cloth
        writer.write_type(&(self.buffers.num_uv_channels() as u32), endian)?;
        align_writer(writer, 16)?;

        *args.2 = writer.stream_position()? as u32;

        writer.write_type(&header_offset, endian)?;
        writer.write_type(&0u32, endian)?; //todo: change this to use align_writer
        writer.write_type(&0u64, endian)?;

        Ok(())
    }
}
