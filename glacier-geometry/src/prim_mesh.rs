use std::io::{Seek, SeekFrom, Write};
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};
use crate::math::Vector4;
use crate::prim_object::PrimObject;
use crate::render_primitive::{align_writer, PrimPropertyFlags};

use crate::prim_sub_mesh::PrimSubMesh;

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags))]
pub struct PrimMesh
{
    pub prim_object: PrimObject,

    #[br(temp)]
    sub_mesh_table_offset: u32,

    #[br(temp)]
    pub pos_scale: Vector4,

    #[br(temp)]
    pub pos_bias: Vector4,

    #[br(temp)]
    pub tex_scale_bias: Vector4,

    #[br(pad_after(3))]
    pub cloth_id: u8,

    #[br(
    parse_with = FilePtr32::parse,
    seek_before = SeekFrom::Start(sub_mesh_table_offset as u64),
    restore_position,
    args{
    inner: binrw::args ! {
    global_properties: global_properties,
    mesh_properties: prim_object.properties,
    cloth_id,
    pos_scale,
    pos_bias,
    tex_scale_bias,
    }
    })]
    pub sub_mesh: PrimSubMesh,
}

impl PrimMesh {
    pub fn calc_pos_scale(&self) -> Vector4 {
        let dimensions = self.sub_mesh.calc_bb().dimensions();
        Vector4 {
            x: dimensions.x / 2.0,
            y: dimensions.y / 2.0,
            z: dimensions.z / 2.0,
            w: 0.5,
            //w: 32767.0,
        }
    }

    pub fn calc_pos_bias(&self) -> Vector4 {
        let center = self.sub_mesh.calc_bb().center();
        Vector4 {
            x: center.x,
            y: center.y,
            z: center.z,
            w: 0.5,
            //w: 0.0,
        }
    }

    pub fn calc_uv_scale_bias(&self) -> Vector4 {
        let center = self.sub_mesh.calc_uv_bb().center();
        let dimensions = self.sub_mesh.calc_uv_bb().dimensions();
        Vector4 {
            x: dimensions.x / 2.0,
            y: dimensions.y / 2.0,
            z: center.x,
            w: center.y,
        }
    }
}

impl BinWrite for PrimMesh {
    type Args<'a> = (&'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let mut sub_mesh_ptr: u32 = 0;
        PrimSubMesh::write_options(&self.sub_mesh, writer, endian, (self, args.0, &mut sub_mesh_ptr))?;

        *args.1 = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_object, writer, endian, (self.sub_mesh.calc_bb(),))?;
        writer.write_type(&sub_mesh_ptr, endian)?; //sub_mesh_offset
        if args.0.has_highres_positions() {
            writer.write_type(&Vector4{ x: 1.0, y: 1.0, z: 1.0, w: 1.0 },endian)?;
            writer.write_type(&Vector4{ x: 0.0, y: 0.0, z: 0.0, w: 0.0 },endian)?;
        }else{
            writer.write_type(&self.calc_pos_scale(), endian)?;
            writer.write_type(&self.calc_pos_bias(), endian)?;
        }
        writer.write_type(&self.calc_uv_scale_bias(), endian)?;
        writer.write_type(&self.cloth_id, endian)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}
