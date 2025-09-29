use crate::utils::io::align_writer;
use crate::model::prim_object::PrimObject;
use crate::utils::math::Vector4;
use crate::mesh::prim_sub_mesh::PrimSubMesh;
use crate::render_primitive::PrimPropertyFlags;
use crate::model::prim_mesh::PrimMesh;
use std::io::{Seek, SeekFrom, Write};
use binrw::{binread, binrw, BinRead, BinResult, BinWrite, BinWriterExt, Endian};
use binrw::file_ptr::NonZeroFilePtr32;
use bit_set::BitSet;
use crate::model::prim_mesh_weighted::{BoneAccel, BoneInfo, CopyBones};
use crate::WoaVersion;

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(woa_version: WoaVersion, global_properties: PrimPropertyFlags))]
pub struct PrimMeshLinked
{
    #[br(args(woa_version, global_properties))]
    pub prim_mesh: PrimMesh,

    #[br(temp)]
    pub num_copy_bones: u32,

    #[br(temp)]
    pub copy_bones_offset: u32,

    #[br(temp)]
    pub _unk2: u32,

    #[br(if(copy_bones_offset != 0),
    seek_before = SeekFrom::Start(copy_bones_offset as u64),
    restore_position,
    args{ count: num_copy_bones })]
    pub copy_bones: Option<CopyBones>,

    #[br(parse_with = NonZeroFilePtr32::parse)]
    #[br(args{inner: (woa_version,)})]
    pub bone_info: BoneInfoHolder,
}

#[binrw]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(woa_version: WoaVersion))]
#[bw(import(bone_coli_offset: &mut u32))]
pub enum BoneInfoHolder{
    #[br(pre_assert(woa_version == WoaVersion::HM2016))]
    Normal(
        #[bw(args(bone_coli_offset))]
        BoneInfo
    ),
    #[br(pre_assert(woa_version != WoaVersion::HM2016))]
    Compact(
        #[bw(args(bone_coli_offset))]
        CompactBoneInfo
    ),
}


#[binrw::parser(reader)]
fn parse_bone_remap(total_chunks_align: u32) -> BinResult<Vec<u8>> {
    let u64_count = (total_chunks_align + 1).div_ceil(64) as usize;
    let mut bitset = vec![255u8; u64_count * 64];

    let values : Vec<u64>= (0..u64_count).flat_map(|_| {
        u64::read_le(reader)
    }).collect::<Vec<_>>();

    let mut idx = 0;
    let mut bone_idx = 0;

    for value in values.iter() {
        for bit in 0..64 {
            if (value & (1 << bit)) != 0 {
                bitset[idx] = bone_idx;
                bone_idx += 1;
            }
            idx += 1;
        }
    }
    Ok(bitset.into_iter().take((total_chunks_align + 1) as usize).collect())
}

fn encode_bone_remap_values(bone_remap: Vec<u8>) -> Vec<u64> {
    let n_bits = bone_remap.len();
    let u64_count = n_bits.div_ceil(64);

    let mut out = Vec::with_capacity(u64_count);
    for word in 0..u64_count {
        let mut value: u64 = 0;
        for bit in 0..64 {
            let idx = word * 64 + bit;
            if idx >= n_bits {
                break;
            }
            if bone_remap.get(idx).copied().unwrap_or(0xFF) != 0xFF {
                value |= 1u64 << bit;
            }
        }
        out.push(value);
    }
    out
}

#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct CompactBoneInfo
{
    #[br(temp)]
    pub _total_size: u16,

    #[br(temp)]
    pub num_blocks: u16,

    #[br(temp)]
    pub total_chunks_align: u32,

    #[br(parse_with = parse_bone_remap, args(total_chunks_align))]
    pub bone_remap: Vec<u8>,

    #[br(little, count = num_blocks)]
    pub accel_entries: Vec<BoneAccel>,
}

impl BinWrite for CompactBoneInfo {
    type Args<'a> = (&'a mut u32,);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        *args.0 = writer.stream_position()? as u32;
         let packed_remap = encode_bone_remap_values(self.bone_remap.clone());
        let total_size = 0x8 /* header size */ + (packed_remap.len() * 8) + (self.accel_entries.len() * size_of::<BoneAccel>());

        (total_size as u16).write_options(writer, endian, ())?;
        (self.accel_entries.len() as u16).write_options(writer, endian, ())?;
        ((self.bone_remap.len()-1) as u32).write_options(writer, endian, ())?;
        packed_remap.write_options(writer, endian, ())?;
        for entry in &self.accel_entries {
            entry.write_options(writer, endian, ())?;
        }
        align_writer(writer, 16)?;

        Ok(())
    }
}

impl BinWrite for PrimMeshLinked {
    type Args<'a> = (&'a WoaVersion, &'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        let mut sub_mesh_ptr: u32 = 0;
        PrimSubMesh::write_options(&self.prim_mesh.sub_mesh, writer, endian, (args.0, &self.prim_mesh, args.1, &mut sub_mesh_ptr))?;

        let mut copy_bones_ptr: u32 = 0;
        if let Some(copy_bones) = &self.copy_bones{
            CopyBones::write_options(copy_bones, writer, endian, (&mut copy_bones_ptr,))?;
        }

        let mut coli_bone_ptr: u32 = 0;
        BoneInfoHolder::write_options(&self.bone_info, writer, endian, (&mut coli_bone_ptr,))?;

        *args.2 = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_mesh.prim_object, writer, endian, (self.prim_mesh.calc_bb(),))?;
        writer.write_type(&sub_mesh_ptr, endian)?; //sub_mesh_offset
        if args.1.has_highres_positions() {
            writer.write_type(&Vector4{ x: 1.0, y: 1.0, z: 1.0, w: 1.0 },endian)?;
            writer.write_type(&Vector4{ x: 0.0, y: 0.0, z: 0.0, w: 0.0 },endian)?;
        }else{
            writer.write_type(&self.prim_mesh.pos_scale, endian)?;
            writer.write_type(&self.prim_mesh.pos_bias, endian)?;
        }
        writer.write_type(&self.prim_mesh.tex_scale_bias, endian)?;
        writer.write_type(&(self.prim_mesh.cloth_id as u32), endian)?;

        writer.write_type(&(self.copy_bones.as_ref().map(|c|c.indices.len()).unwrap_or_default() as u32), endian)?;
        writer.write_type(&copy_bones_ptr, endian)?;
        writer.write_type(&0u32, endian)?;
        writer.write_type(&coli_bone_ptr, endian)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}




impl PrimMeshLinked {

    pub fn bone_remap(&self) -> Vec<u8> {
        match &self.bone_info{
            BoneInfoHolder::Normal(normal) => {normal.bone_remap.clone()},
            BoneInfoHolder::Compact(compact) => {compact.bone_remap.clone()},
        }
    }

    pub fn accel_entries(&self) -> Vec<BoneAccel> {
        match &self.bone_info{
            BoneInfoHolder::Normal(normal) => {normal.accel_entries.clone()},
            BoneInfoHolder::Compact(compact) => {compact.accel_entries.clone()},
        }
    }

    pub fn get_indices_for_bone(&self, bone_index: usize) -> Option<Vec<u16>>{
        let accel_entries = self.accel_entries();
        let bone_remap = self.bone_remap();
        let accel_entry = accel_entries.get(*bone_remap.get(bone_index)? as usize)?;
        let indices = (0..accel_entry.num_indices as usize).map(|i| self.prim_mesh.sub_mesh.indices.get(accel_entry.offset as usize + i)).flatten().copied().collect::<Vec<_>>();
        Some(indices)
    }
}
