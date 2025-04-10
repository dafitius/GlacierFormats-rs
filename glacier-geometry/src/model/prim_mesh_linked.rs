use crate::utils::io::align_writer;
use crate::model::prim_object::PrimObject;
use crate::utils::math::Vector4;
use crate::mesh::prim_sub_mesh::PrimSubMesh;
use crate::render_primitive::PrimPropertyFlags;
use crate::model::prim_mesh::PrimMesh;
use std::io::{Read, Seek, Write};
use binrw::{binread, BinRead, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};
use bit_set::BitSet;
use itertools::Itertools;
use crate::model::prim_mesh_weighted::{BoneAccel, BoneIndices, CopyBones, PrimMeshWeighted};

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags))]
pub struct PrimMeshLinked
{
    #[br(args(global_properties))]
    pub prim_mesh: PrimMesh,

    #[br(temp)]
    pub unk_0: u32,

    #[br(temp)]
    pub unk_1: u32,

    #[br(temp)]
    pub unk_2: u32,

    #[br(parse_with = FilePtr32::parse)]
    pub bone_info: BoneInfo,
}


#[binrw::parser(reader, endian)]
fn parse_bone_remap(total_chunks_align: u32) -> BinResult<BitSet> {
    let mut bitset = BitSet::with_capacity(total_chunks_align as usize);
    let u64_count = ((total_chunks_align as f32 + 0.001) / 64.0).ceil() as usize;

    let values : Vec<u64>= (0..u64_count).map(|_| {
        u64::read_le(reader)
    }).flatten().collect::<Vec<_>>();

    for (i, value) in values.iter().rev().enumerate() {
        for bit in 0..64 {
            if (value & (1 << bit)) != 0 {
                bitset.insert(i * 64 + (64 - bit));
            }
        }
    }
    let size = bitset.get_ref().len();
    Ok(BitSet::from_bit_vec(bitset.into_bit_vec().iter().skip(size - total_chunks_align as usize).collect()))
}


fn bitset_to_bytes(bitset: &BitSet) -> BinResult<Vec<u8>> {
    let mut buffer = vec![];
    let bit_vec = bitset.clone().into_bit_vec();
    let size = bit_vec.len();
    let aligned_size = ((size + 63) / 64) + size;
    let mut values = vec![0u64; (aligned_size + 63) / 64];

    for (i, bit) in bit_vec.iter().enumerate() {
        if bit {
            let value_index = i / 64;
            let bit_index = i % 64;
            values[value_index] |= 1 << bit_index;
        }
    }

    for value in values.iter().rev() {
        let reversed_value = reverse_u64_bits(*value);
        buffer.write_all(&reversed_value.to_le_bytes())?;
    }

    Ok(buffer)
}

fn reverse_u64_bits(mut value: u64) -> u64 {
    let mut reversed = 0u64;
    for _ in 0..64 {
        reversed <<= 1;
        reversed |= value & 1;
        value >>= 1;
    }
    reversed
}



#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct BoneInfo
{
    #[br(temp)]
    pub total_size: u16,

    #[br(temp)]
    pub num_blocks: u16,

    // #[br(temp)]
    pub total_chunks_align: u32,

    // #[br(parse_with = parse_bone_remap, args(total_chunks_align))]
    // #[br(dbg)]
    // pub bone_remap: BitSet,

    #[br(count = (total_chunks_align + 63) / 64)]
    pub bone_remap: Vec<u64>,

    #[br(little, count = num_blocks)]
    pub accel_entries: Vec<BoneAccel>,
}

impl BinWrite for BoneInfo {
    type Args<'a> = &'a mut u32;

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        *args = writer.stream_position()? as u32;

        // let bit_vec = self.bone_remap.clone().into_bit_vec();
        // let size : u32 = bit_vec.len() as u32;
        // let aligned_size: u32 = ((size + 63) / 64);

        // let total_size = 0x8 /* header size */ + (aligned_size * size_of::<u64>() as u32) + (self.accel_entries.len() * size_of::<BoneAccel>()) as u32;
        let total_size = 0x8 /* header size */ + (self.bone_remap.len() * size_of::<u64>()) + (self.accel_entries.len() * size_of::<BoneAccel>());

        (total_size as u16).write_options(writer, endian, ())?;
        (self.accel_entries.len() as u16).write_options(writer, endian, ())?;
        // size.write_options(writer, endian, ())?;
        self.total_chunks_align.write_options(writer, endian, ())?;
        self.bone_remap.write_options(writer, endian, ())?;
        // bitset_to_bytes(&self.bone_remap)?.write_options(writer, endian, ())?;
        for entry in &self.accel_entries {
            entry.write_options(writer, endian, ())?;
        }
        align_writer(writer, 16)?;

        Ok(())
    }
}

impl BinWrite for PrimMeshLinked {
    type Args<'a> = (&'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        let mut sub_mesh_ptr: u32 = 0;
        PrimSubMesh::write_options(&self.prim_mesh.sub_mesh, writer, endian, (&self.prim_mesh, args.0, &mut sub_mesh_ptr))?;

        let mut coli_bone_ptr: u32 = 0;
        BoneInfo::write_options(&self.bone_info, writer, endian, &mut coli_bone_ptr)?;

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

        writer.write_type(&0u32, endian)?;
        writer.write_type(&0u32, endian)?;
        writer.write_type(&0u32, endian)?;
        writer.write_type(&coli_bone_ptr, endian)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}




impl PrimMeshLinked {
    // pub fn get_indices_for_bone(&self, bone_index: usize) -> Option<Vec<u16>>{
    //     if self.bone_info.bone_remap.get_ref().get(bone_index)? {
    //         let entry_index = self.bone_info.bone_remap.get_ref().iter().enumerate().filter(|(i, b)| i <= &bone_index && *b).count();
    //         let accel_entry = self.bone_info.accel_entries.get(entry_index)?;
    //         let indices = (0..accel_entry.num_indices as usize).map(|i| self.prim_mesh.sub_mesh.indices.get(accel_entry.offset as usize + i)).flatten().copied().collect::<Vec<_>>();
    //         Some(indices)
    //     } else {
    //         None
    //     }
    // }
}
