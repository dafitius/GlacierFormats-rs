use crate::model::prim_object::PrimObject;
use crate::utils::math::Vector4;
use crate::render_primitive::align_writer;
use crate::mesh::prim_sub_mesh::PrimSubMesh;
use crate::render_primitive::PrimPropertyFlags;
use crate::model::prim_mesh::PrimMesh;
use std::io::{Seek, SeekFrom, Write};
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};
use binrw::file_ptr::NonZeroFilePtr32;
use itertools::Itertools;
use crate::model::prim_mesh_linked::PrimMeshLinked;

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags))]
pub struct PrimMeshWeighted
{
    #[br(args(global_properties))]
    pub prim_mesh: PrimMesh,

    #[br(temp)]
    pub num_copy_bones: u32,

    #[br(temp)]
    pub copy_bones_offset: u32,

    #[br(if(copy_bones_offset != 0),
    seek_before = SeekFrom::Start(copy_bones_offset as u64),
    restore_position,
    args{ count: num_copy_bones })]
    pub copy_bones: Option<CopyBones>,

    #[br(temp)]
    pub bone_indices_offset: u32,

    #[br(seek_before = SeekFrom::Start(bone_indices_offset as u64),
    restore_position
    )]
    pub bone_indices: BoneIndices,

    #[br(parse_with = NonZeroFilePtr32::parse)]
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
        BoneIndices::write_options(&self.bone_indices, writer, endian, &mut bone_indices_ptr)?;

        *args.1 = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_mesh.prim_object, writer, endian, (self.prim_mesh.calc_bb(),))?;
        writer.write_type(&sub_mesh_ptr, endian)?; //sub_mesh_offset
        if args.0.has_highres_positions() {
            writer.write_type(&Vector4{ x: 1.0, y: 1.0, z: 1.0, w: 1.0 },endian)?;
            writer.write_type(&Vector4{ x: 0.0, y: 0.0, z: 0.0, w: 0.0 },endian)?;
        }else{
            writer.write_type(&self.prim_mesh.pos_scale, endian)?;
            writer.write_type(&self.prim_mesh.pos_bias, endian)?;
        }
        writer.write_type(&self.prim_mesh.tex_scale_bias, endian)?;
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
#[derive(Debug, BinWrite, Default, PartialEq, Clone)]
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
#[derive(Debug, PartialEq, Clone)]
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
#[derive(Debug, PartialEq, Clone)]
pub struct BoneInfo
{
    #[br(temp)]
    pub total_size: u16,

    #[br(temp)]
    pub num_accel_entries: u16,

    #[br(pad_after(1), count = 255)]
    pub bone_remap: Vec<u8>,

    #[br(little, count = num_accel_entries)]
    pub accel_entries: Vec<BoneAccel>,
}

impl BinWrite for BoneInfo{
    type Args<'a> = &'a mut u32;

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        *args = writer.stream_position()? as u32;

        let total_size = 4 +
            (self.bone_remap.len() + 1) +
            (self.accel_entries.len() * 8);

        writer.write_type(&(total_size as u16), endian)?;
        writer.write_type(&(self.accel_entries.len() as u16), endian)?;

        writer.write_type(&self.bone_remap, endian)?;
        writer.write_type(&0u8, endian)?;

        writer.write_type(&self.accel_entries, endian)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}

#[binread]
#[derive(Debug, BinWrite, PartialEq, Clone)]
pub struct BoneAccel
{
    pub offset: u32,
    pub num_indices: u32,
}

impl PrimMeshWeighted {
    pub fn get_indices_for_bone(&self, bone_index: usize) -> Option<Vec<u16>>{

        // for weights in self.prim_mesh.get_weights() {
        //     for weight in weights{
        //         for idx in weight.indices.iter(){
        //             let bone_idx: &u8 = self.bone_info.bone_remap?.get(idx)?;
        //             if bone_idx == bone_index {
        //
        //             }
        //         }
        //     }
        // }
        //
        // if self.bone_info.bone_remap.get_ref().get(bone_index)? {
        //     let entry_index = self.bone_info.bone_remap.get_ref().iter().enumerate().filter(|(i, b)| i <= &bone_index && *b).count();
        //     let accel_entry = self.bone_info.accel_entries.get(entry_index)?;
        //     let indices = (0..accel_entry.num_indices as usize).map(|i| self.prim_mesh.sub_mesh.indices.get(accel_entry.offset as usize + i)).flatten().copied().collect::<Vec<_>>();
        //     Some(indices)
        // } else {
        //     None
        // }
        None
    }
}