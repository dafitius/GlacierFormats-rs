use crate::mesh::prim_sub_mesh::PrimSubMesh;
use crate::model::prim_mesh::PrimMesh;
use crate::model::prim_object::PrimObject;
use crate::render_primitive::PrimPropertyFlags;
use crate::utils::io::align_writer;
use crate::utils::math::Vector4;
use crate::WoaVersion;
use binrw::file_ptr::NonZeroFilePtr32;
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian};
use itertools::Itertools;
use num_traits::Zero;
use std::io::{Seek, SeekFrom, Write};
use std::num::NonZeroU8;
use std::ptr::NonNull;

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(woa_version: WoaVersion, global_properties: PrimPropertyFlags))]
pub struct PrimMeshWeighted {
    #[br(args(woa_version, global_properties))]
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
    type Args<'a> = (&'a WoaVersion, &'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let mut sub_mesh_ptr: u32 = 0;
        PrimSubMesh::write_options(
            &self.prim_mesh.sub_mesh,
            writer,
            endian,
            (args.0, &self.prim_mesh, args.1, &mut sub_mesh_ptr),
        )?;

        let mut bone_info_ptr: u32 = 0;
        BoneInfo::write_options(&self.bone_info, writer, endian, (&mut bone_info_ptr,))?;

        let mut bone_indices_ptr: u32 = 0;
        BoneIndices::write_options(&self.bone_indices, writer, endian, &mut bone_indices_ptr)?;

        *args.2 = writer.stream_position()? as u32;
        PrimObject::write_options(
            &self.prim_mesh.prim_object,
            writer,
            endian,
            (self.prim_mesh.calc_bb(),),
        )?;
        writer.write_type(&sub_mesh_ptr, endian)?; //sub_mesh_offset
        if args.1.has_highres_positions() {
            writer.write_type(
                &Vector4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                },
                endian,
            )?;
            writer.write_type(
                &Vector4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
                endian,
            )?;
        } else {
            writer.write_type(&self.prim_mesh.pos_scale, endian)?;
            writer.write_type(&self.prim_mesh.pos_bias, endian)?;
        }
        writer.write_type(&self.prim_mesh.tex_scale_bias, endian)?;
        writer.write_type(&(self.prim_mesh.cloth_id as u32), endian)?;

        writer.write_type(
            &match &self.copy_bones {
                None => 0u32,
                Some(copy_bones) => copy_bones.len(),
            },
            endian,
        )?;
        writer.write_type(&0u32, endian)?; //copy bones, always zero for weighted meshes
        writer.write_type(&bone_indices_ptr, endian)?; //bone indices offset
        writer.write_type(&bone_info_ptr, endian)?; //bone info offset
        align_writer(writer, 16)?;

        Ok(())
    }
}

#[binread]
#[derive(Debug, Default, PartialEq, Clone)]
#[br(import{count: u32})]
pub struct CopyBones {
    #[brw(count = count as usize)]
    pub indices: Vec<u32>,

    #[brw(count = count as usize)]
    pub offsets: Vec<u32>,
}

impl CopyBones {
    pub fn len(&self) -> u32 {
        self.indices.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

impl BinWrite for CopyBones {
    type Args<'a> = (&'a mut u32,);

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        *args.0 = writer.stream_position()? as u32;

        self.indices.write_le(writer)?;
        self.offsets.write_le(writer)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}

#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct BoneIndices {
    #[br(temp)]
    pub count: u32,

    #[brw(count = count as usize - 2)]
    pub indices: Vec<u16>,
}

impl BoneIndices {
    pub fn split_groups(&self) -> Vec<Vec<u16>> {
        let mut result = Vec::new();
        let mut iter = self.indices.iter();

        while let Some(&size) = iter.next() {
            let group: Vec<u16> = iter.by_ref().take((size - 1) as usize).copied().collect();
            result.push(group);
        }
        result
    }
}

impl BinWrite for BoneIndices {
    type Args<'a> = &'a mut u32;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        *args = writer.stream_position()? as u32;
        writer.write_type(&((self.indices.len() + 2) as u32), endian)?;
        writer.write_type(&self.indices, endian)?;
        align_writer(writer, 16)?;
        Ok(())
    }
}

#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct BoneInfo {
    #[br(temp)]
    pub _total_size: u16,

    #[br(temp)]
    pub num_accel_entries: u16,

    #[br(pad_after(1), count = 255)]
    pub bone_remap: Vec<u8>,

    #[br(little, count = num_accel_entries)]
    pub accel_entries: Vec<BoneAccel>,
}

impl BinWrite for BoneInfo {
    type Args<'a> = (&'a mut u32,);

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        *args.0 = writer.stream_position()? as u32;

        let total_size = 4 + (self.bone_remap.len() + 1) + (self.accel_entries.len() * 8);

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
pub struct BoneAccel {
    pub offset: u32,
    pub num_indices: u32,
}

impl PrimMeshWeighted {
    //The bone index in vertices cannot be 0
    pub fn indices_for_bone(&self, bone_index: u8) -> Option<&[u16]> {
        let bone_map = &self.bone_info.bone_remap;
        let accel_idx = bone_map.get(bone_index as usize)?;
        if *accel_idx == 0xFF {
            return None;
        }

        let entry = self.bone_info.accel_entries.get(*accel_idx as usize)?;
        let start = entry.offset as usize - 2; // :(
        let len = entry.num_indices as usize;
        let end = start.checked_add(len)?;

        self.bone_indices.indices.get(start..end)
    }

    pub fn triangles_for_bone(&self, bone_index: u8) -> Option<Vec<&[u16; 3]>> {
        self.indices_for_bone(bone_index).map(|indices| {
            indices
                .chunks(3)
                .flat_map(|chunk: &[u16]| chunk.try_into())
                .collect_vec()
        })
    }
    
    pub fn repack(&mut self) {
        let (new_indices, new_accel, new_remap) = self.pack_bones();
        self.bone_indices = new_indices;
        self.bone_info.bone_remap = new_remap;
        self.bone_info.accel_entries = new_accel;
    }

    pub fn pack_bones(&self) -> (BoneIndices, Vec<BoneAccel>, Vec<u8>) {
        let vertex_count = self.prim_mesh.sub_mesh.num_vertices as usize;

        let mut main_bone_per_vertex: Vec<Option<u16>> = Vec::with_capacity(vertex_count);
        for v_weights in self.prim_mesh.get_weights().expect("weights stream") {
            // Pick the bone with maximum weight among nonzero bone+weight entries
            let mut best: Option<(u16, f32)> = None;
            for (bone, weight) in v_weights.iter().copied() {
                if !bone.is_zero() && !weight.is_zero() {
                    let b = bone as u16;
                    let w = weight as f32;
                    match best {
                        Some((_, bw)) if bw >= w => {}
                        _ => best = Some((b, w)),
                    }
                }
            }
            main_bone_per_vertex.push(best.map(|(b, _)| b));
        }

        use std::collections::{BTreeMap, BTreeSet};
        let mut groups_by_bone: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
        
        for tri in self.prim_mesh.get_triangles() {
            let mut uniq: BTreeSet<u16> = BTreeSet::new();
            for &vidx in tri {
                if let Some(Some(bone)) = main_bone_per_vertex.get(vidx as usize) {
                    uniq.insert(*bone);
                }
            }
            if uniq.is_empty() {
                continue;
            }
            for bone in uniq {
                groups_by_bone
                    .entry(bone)
                    .or_default()
                    .extend_from_slice(tri);
            }
        }
        
        let mut bone_remap: Vec<u8> = vec![0xFF; 255];

        let mut accel_index: u8 = 0;
        for (&bone, tris) in groups_by_bone.iter() {
            if !tris.is_empty() {
                bone_remap[bone as usize] = accel_index;
                accel_index = accel_index.wrapping_add(1);
            }
        }
        let num_accel = accel_index as usize;
        
        let mut groups: Vec<Vec<u16>> = vec![Vec::new(); num_accel];
        for (bone, tris) in groups_by_bone {
            let aid = bone_remap[bone as usize];
            if aid != 0xFF {
                groups[aid as usize] = tris;
            }
        }
        
        let mut flat: Vec<u16> = Vec::new();
        let mut accels: Vec<BoneAccel> = Vec::with_capacity(num_accel);
        let mut offset: u32 = 0;

        for g in groups {
            let tri_words = g.len() as u16;
            let group_len = tri_words + 1;

            let stored_offset = offset + 2;
            let num_indices = tri_words as u32;
            
            flat.push(group_len);
            flat.extend_from_slice(&g);

            accels.push(BoneAccel {
                offset: stored_offset + 1,
                num_indices,
            });
            offset += group_len as u32;
        }
        (BoneIndices { indices: flat }, accels, bone_remap)
    }

}
