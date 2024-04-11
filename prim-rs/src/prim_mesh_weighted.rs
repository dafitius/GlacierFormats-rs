use std::io::{Seek, Write};
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};

use crate::math::Vector4;
use crate::render_primitive::{align_writer, PrimPropertyFlags};

#[cfg(feature = "serde")]
use serde::{Serialize};

use crate::prim_mesh::PrimMesh;
use crate::prim_object::PrimObject;
use crate::prim_sub_mesh::PrimSubMesh;

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[br(import(global_properties: PrimPropertyFlags))]
pub struct PrimMeshWeighted
{
    #[br(args(global_properties))]
    pub prim_mesh: PrimMesh,

    #[br(temp)]
    pub num_copy_bones: u32,

    #[br(temp, restore_position)]
    pub copy_bones_offset: u32,

    #[br(temp, if(copy_bones_offset == 0))]
    pub pad: u32,

    #[br(if(copy_bones_offset != 0), parse_with = FilePtr32::parse,
    args{inner: binrw::args ! { count: num_copy_bones }})]
    pub copy_bones: Option<CopyBones>,

    #[br(temp, restore_position)]
    pub bone_indices_offset: u32,

    #[br(temp, if(bone_indices_offset == 0))]
    pub pad: u32,

    #[br(if(bone_indices_offset != 0), parse_with = FilePtr32::parse)]
    pub bone_indices: Option<BoneIndices>,

    #[br(parse_with = FilePtr32::parse,
    args{inner: binrw::args ! {
    global_properties,
    num_copy_bones
    }})]
    pub bone_info: BoneInfo,
}

impl BinWrite for PrimMeshWeighted {
    type Args<'a> = (&'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        let mut sub_mesh_ptr: u32 = 0;
        PrimSubMesh::write_options(&self.prim_mesh.sub_mesh, writer, endian, (&self.prim_mesh, args.0, &mut sub_mesh_ptr))?;

        let mut bone_info_ptr: u32 = 0;
        BoneInfo::write_options(&self.bone_info, writer, endian, &mut bone_info_ptr)?;


        let mut bone_indices_ptr: u32 = 0;
        if let Some(bone_indices) = &self.bone_indices{
            BoneIndices::write_options(bone_indices, writer, endian, &mut bone_indices_ptr)?;
        }

        *args.1 = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_mesh.prim_object, writer, endian, (self.prim_mesh.sub_mesh.calc_bb(),))?;
        writer.write_type(&sub_mesh_ptr, endian)?; //sub_mesh_offset
        if args.0.has_highres_positions() {
            writer.write_type(&Vector4{ x: 1.0, y: 1.0, z: 1.0, w: 1.0 },endian)?;
            writer.write_type(&Vector4{ x: 0.0, y: 0.0, z: 0.0, w: 0.0 },endian)?;
        }else{
            writer.write_type(&self.prim_mesh.calc_pos_scale(), endian)?;
            writer.write_type(&self.prim_mesh.calc_pos_bias(), endian)?;
        }
        writer.write_type(&self.prim_mesh.calc_uv_scale_bias(), endian)?;
        writer.write_type(&(self.prim_mesh.cloth_id as u32), endian)?;


        writer.write_type(&match &self.copy_bones{
            None => {0u32}
            Some(copy_bones) => {copy_bones.len()}
        }, endian)?;
        writer.write_type(&0u32, endian)?; //copy bones, always zero for weighted meshes
        writer.write_type(&bone_indices_ptr, endian)?; //bone indices offset
        writer.write_type(&bone_info_ptr, endian)?; //bone info offset
        align_writer(writer, 16)?;

        Ok(())
    }
}


#[binread]
#[derive(Debug, BinWrite, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[br(import{count: u32})]
pub struct CopyBones
{
    #[brw(count = count as usize)]
    pub indices: Vec<u32>,

    #[brw(count = count as usize)]
    pub offsets: Vec<u32>,
}

impl CopyBones {
    pub fn len(&self) -> u32{
        self.indices.len() as u32
    }
}

#[binread]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct BoneIndices
{
    #[br(temp)]
    pub count: u32,

    #[brw(count = count as usize)]
    pub indices: Vec<u16>,
}

impl BinWrite for BoneIndices {
    type Args<'a> = &'a mut u32;

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        *args = writer.stream_position()? as u32;
        writer.write_type(&(self.indices.len() as u32), endian)?;
        writer.write_type(&self.indices, endian)?;
        align_writer(writer, 16)?;
        Ok(())
    }
}

#[binread]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[br(import{global_properties: PrimPropertyFlags, num_copy_bones: u32})]
pub struct BoneInfo
{
    #[br(temp)]
    pub total_size: u16,

    #[br(temp)]
    pub num_accel_entries: u16,

    #[br(if((num_copy_bones > 0) || global_properties.is_weighted_object()), pad_after(1), count = 255)]
    pub bone_remap: Option<Vec<u8>>,

    #[br(if((num_copy_bones == 0) && !global_properties.is_weighted_object()), count = 3)]
    pub unknown: Option<Vec<u32>>,

    #[br(little, count = num_accel_entries)]
    pub accel_entries: Vec<BoneAccel>,
}

impl BinWrite for BoneInfo{
    type Args<'a> = &'a mut u32;

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        *args = writer.stream_position()? as u32;

        let total_size = 4 +
            (if self.bone_remap.is_some() {256} else {0}) +
            (if self.unknown.is_some() {12} else {0}) +
            (self.accel_entries.len() * 8);

        writer.write_type(&(total_size as u16), endian)?;
        writer.write_type(&(self.accel_entries.len() as u16), endian)?;

        if let Some(bone_remap) = &self.bone_remap{
            writer.write_type(bone_remap, endian)?;
            writer.write_type(&0u8, endian)?;
        }

        if let Some(unknown) = &self.unknown{
            writer.write_type(unknown, endian)?;
        }

        writer.write_type(&self.accel_entries, endian)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}

#[binread]
#[derive(Debug, BinWrite, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct BoneAccel
{
    pub offset: u32,
    pub num_indices: u32,
}