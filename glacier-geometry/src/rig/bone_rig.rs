use std::{fs, io};
use std::fs::File;
use crate::utils::math::{Matrix43, Quaternion, Transform, Vector3, Vector4};
use binrw::{binread, binrw, BinRead, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32, NullString};
use std::io::{BufReader, BufWriter, Cursor, Seek, SeekFrom, Write};
use std::path::Path;
use itertools::Itertools;
use crate::utils::io::{align_writer, FixedString};

#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
struct BoneRigHeader {
    number_of_bones: u32,
    number_of_animated_bones: u32,
    bone_definitions_offset: u32,
    bind_pose_offset: u32,
    bind_pose_inv_global_mats_offset: u32,
    bone_constraints_header_offset: u32,
    pose_bone_header_offset: u32,

    // invert_global_bones and bone_map are both unused (0) pointers likely leftover from an old version of the BoneRig
    invert_global_bones_offset: u32,
    bone_map_offset: u64,
}

#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct BoneRig {
    #[br(temp)]
    header_offset: u64,

    #[br(seek_before(SeekFrom::Start(header_offset)), temp)]
    header: BoneRigHeader,

    #[br(calc(header.number_of_animated_bones))]
    #[bw(skip)]
    pub num_animated_bones: u32,

    #[br(
        count = header.number_of_bones,
        seek_before(SeekFrom::Start(header.bone_definitions_offset as u64)),
        restore_position
    )]
    pub bone_definitions: Vec<BoneDefinition>,

    #[br(
        count = header.number_of_bones,
        seek_before(SeekFrom::Start(header.bind_pose_offset as u64)),
        restore_position
    )]
    pub bind_pose: Vec<Transform>,

    #[br(
        count = header.number_of_bones,
        seek_before(SeekFrom::Start(header.bind_pose_inv_global_mats_offset as u64)),
        restore_position
    )]
    pub bind_pose_inv_global_mats: Vec<Matrix43>,

    #[br(
        seek_before(SeekFrom::Start(header.bone_constraints_header_offset as u64)),
        restore_position
    )]
    pub bone_constraints: BoneConstraints,

    #[br(
        seek_before(SeekFrom::Start(header.pose_bone_header_offset as u64)),
        restore_position
    )]
    pub pose_bone_info: PoseBoneInfo,
}

impl BinWrite for BoneRig {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        writer.write_le(&0u64)?;
        writer.write_le(&0u64)?;

        let mut pose_bone_header_offset = 0;
        writer.write_le_args(&self.pose_bone_info, &mut pose_bone_header_offset)?;
        align_writer(writer, 16)?;

        let bone_definitions_offset = writer.stream_position()? as u32;
        writer.write_le(&self.bone_definitions)?;
        align_writer(writer, 16)?;

        let bind_pose_offset = writer.stream_position()? as u32;
        writer.write_le(&self.bind_pose)?;
        align_writer(writer, 16)?;

        let bind_pose_inv_global_mats_offset = writer.stream_position()? as u32;
        writer.write_le(&self.compute_inv_global_matrices())?;
        align_writer(writer, 16)?;

        let bone_constraints_header_offset = writer.stream_position()? as u32;
        writer.write_le(&self.bone_constraints)?;
        align_writer(writer, 16)?;

        let header = BoneRigHeader{
            number_of_bones: self.bone_definitions.len() as u32,
            number_of_animated_bones: self.num_animated_bones, //TODO: Figure out how to handle this!
            bone_definitions_offset,
            bind_pose_offset,
            bind_pose_inv_global_mats_offset,
            bone_constraints_header_offset,
            pose_bone_header_offset,
            invert_global_bones_offset: 0,
            bone_map_offset: 0,
        };

        let header_offset = writer.stream_position()?;
        writer.write_le(&header)?;
        align_writer(writer, 16)?;

        writer.seek(SeekFrom::Start(0))?;
        writer.write_le(&header_offset)?;

        Ok(())
    }
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Clone)]
pub struct BoneDefinition {
    pub center: Vector3,
    #[br(map = |v: i32| if v >= 0 { Some(v as usize)} else {None})]
    #[bw(map = |opt| opt.map(|p| if p < i32::MAX.try_into().unwrap() { p as i32 } else { -1i32 }).unwrap_or(-1i32))]
    pub prev_bone_nr: Option<usize>, //A prev_bone_nr can't be more than a i32
    pub size: Vector3,
    #[br(count = 34)]
    pub name: FixedString,
    pub body_part: i16,
}

#[binrw]
#[derive(Debug, PartialEq, Clone)]
pub struct BoneConstraints{
    #[br(temp)]
    #[bw(calc(constraints.len() as u32))]
    count: u32,
    #[br(count = count)]
    pub constraints: Vec<BoneConstraint>
}

#[binrw]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct BoneConstraint {
    #[br(temp)]
    #[bw(calc(match constraint {Constraint::LookAt(_) => { 1 }Constraint::Rotation(_) => { 2 }}))]
    type_id: u8,
    pub bone_id: u8,
    #[br(args(type_id))]
    pub constraint: Constraint,
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
#[br(import(type_id: u8))]
pub enum Constraint {
    #[br(pre_assert(type_id == 1))]
    LookAt(BoneConstraintLookAt),
    #[br(pre_assert(type_id == 2))]
    Rotation(BoneConstraintRotate),
}

#[binrw]
#[derive(Debug, PartialEq, Clone, Copy)]
#[br(assert(nr_targets == 1 || nr_targets == 2))]
pub struct BoneConstraintLookAt {
    #[br(temp)]
    #[bw(calc( match self.targets() {TargetConfig::Single(_) => {1} TargetConfig::Double(_) => {2}}))]
    pub nr_targets: u8,
    look_at_axis: u8,

    up_bone_alignment_axis: u8,
    look_at_flip: u8,
    up_flip: u8,
    upnode_control: u8,

    up_node_parent_idx: u8,
    #[brw(pad_after = 1)]
    target_parent_idx: [u8; 2],

    bone_targets_weights: [f32; 2],
    target_pos: [Vector3; 2],
    up_pos: Vector3,
}

impl BoneConstraintLookAt {
    pub fn targets(&self) -> TargetConfig{
        let target1 = BoneConstraintLookAtTarget::new(self.target_parent_idx[0], self.bone_targets_weights[0], self.target_pos[0]);
        let target2 = BoneConstraintLookAtTarget::new(self.target_parent_idx[1], self.bone_targets_weights[1], self.target_pos[1]);
        if target2 == BoneConstraintLookAtTarget::default() {
            TargetConfig::Single(target1)
        }else {
            TargetConfig::Double((target1, target2))
        }
    }
}

#[derive(Debug)]
pub enum TargetConfig{
    Single(BoneConstraintLookAtTarget),
    Double((BoneConstraintLookAtTarget, BoneConstraintLookAtTarget))
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct BoneConstraintLookAtTarget {
    parent_idx: u8,
    weight: f32,
    position: Vector3,
}

impl BoneConstraintLookAtTarget {
    pub fn new(parent_idx: u8, weight: f32, position: Vector3) -> BoneConstraintLookAtTarget {
        Self{ parent_idx, weight, position }
    }
}


#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
pub struct BoneConstraintRotate {
    #[brw(pad_after = 1)]
    reference_bone_idx: u8,
    twist_weight: f32,
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy, Default)]
pub struct PoseBoneHeader {
    pose_bone_array_offset: u32,
    pose_bone_index_array_offset: u32,
    pose_bone_count_total: u32,

    pose_entry_index_array_offset: u32,
    pose_bone_count_array_offset: u32,
    pose_count: u32,

    names_list_offset: u32,
    names_entry_index_array_offset: u32,

    face_bone_index_array_offset: u32,
    face_bone_count: u32,
}

#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
pub struct PoseBone {
    pub quat: Quaternion,
    pub pos: Vector4,
    pub scale: Vector4,
}

#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct PoseBoneInfo{
    #[br(temp)]
    header: PoseBoneHeader,

    #[br(
        count = header.pose_bone_count_total,
        seek_before(SeekFrom::Start(header.pose_bone_array_offset as u64)),
        restore_position
    )]
    pose_bones: Vec<PoseBone>,

    #[br(
        count = header.pose_bone_count_total,
        seek_before(SeekFrom::Start(header.pose_bone_index_array_offset as u64)),
        restore_position
    )]
    pose_bone_indices: Vec<u32>,

    #[br(
        count = header.pose_count,
        seek_before(SeekFrom::Start(header.pose_entry_index_array_offset as u64)),
        restore_position
    )]
    pose_entry_indices: Vec<u32>,

    #[br(
        count = header.pose_count,
        seek_before(SeekFrom::Start(header.pose_bone_count_array_offset as u64)),
        restore_position
    )]
    pose_bones_counts: Vec<u32>,

    #[br(
        count = header.pose_count,
        seek_before(SeekFrom::Start(header.names_entry_index_array_offset as u64)),
        restore_position
    )]
    names_entry_indices: Vec<u32>,

    #[br(
        count = header.pose_count,
        seek_before(SeekFrom::Start(header.names_list_offset as u64)),
        restore_position
    )]
    pose_name_list: Vec<NullString>,

    #[br(
        count = header.face_bone_count,
        seek_before(SeekFrom::Start(header.face_bone_index_array_offset as u64)),
        restore_position
    )]
    face_bones: Vec<u32>,
}

impl BinWrite for PoseBoneInfo{
    type Args<'a> = (&'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        let pose_bone_array_offset = writer.stream_position()? as u32;
        writer.write_le(&self.pose_bones)?;
        align_writer(writer, 16)?;

        let pose_bone_index_array_offset = writer.stream_position()? as u32;
        writer.write_le(&self.pose_bone_indices)?;
        align_writer(writer, 16)?;

        let pose_entry_index_array_offset = writer.stream_position()? as u32;
        writer.write_le(&self.pose_entry_indices)?;
        align_writer(writer, 16)?;

        let pose_bone_count_array_offset = writer.stream_position()? as u32;
        writer.write_le(&self.pose_bones_counts)?;
        align_writer(writer, 16)?;

        let names_list_offset = writer.stream_position()? as u32;
        writer.write_le(&self.pose_name_list)?;
        align_writer(writer, 16)?;

        let names_entry_index_array_offset = writer.stream_position()? as u32;
        let sizes = self.pose_name_list.iter().enumerate().fold(vec![], |mut acc, (i, name)| {
            if i == 0 { acc.push(0); return acc; }
            let last_size = acc.last().unwrap_or(&0u32);
            let last_name = self.pose_name_list.get(i-1).map(|v|v.len()).unwrap_or(0);
            acc.push( (last_size + last_name as u32 + 1));
            acc
        });

        writer.write_le(&sizes)?;
        align_writer(writer, 16)?;

        let face_bone_index_array_offset = writer.stream_position()? as u32;
        writer.write_le(&self.face_bones)?;
        align_writer(writer, 16)?;

        let header = if self.pose_bones.len() > 0 && self.pose_name_list.len() > 0{ //TODO: add better checking
            PoseBoneHeader {
                pose_bone_array_offset,
                pose_bone_index_array_offset,
                pose_bone_count_total: self.pose_bones.len() as u32,
                pose_entry_index_array_offset,
                pose_bone_count_array_offset,
                pose_count: self.pose_name_list.len() as u32,
                names_list_offset,
                names_entry_index_array_offset,
                face_bone_index_array_offset,
                face_bone_count: self.face_bones.len() as u32,
            }
        } else {
            PoseBoneHeader::default()
        };


        *args = writer.stream_position()? as u32;
        writer.write_le(&header)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}


pub struct PoseData<'a> {
    pub pose_name: String,
    pub bones: Vec<(&'a BoneDefinition, &'a PoseBone)>,
}


#[derive(Debug, thiserror::Error)]
pub enum BoneRigError {
    #[error("Io error")]
    IoError(#[from] io::Error),

    #[error("Parsing error")]
    ParsingError(#[from] binrw::Error),

    #[error("Failed on {0}")]
    UnknownError(String),

    #[error("Failed to serialize: {0}")]
    SerializationError(String),
}

impl BoneRig{

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, BoneRigError> {
        let file = File::open(path).map_err(BoneRigError::IoError)?;
        let mut reader = BufReader::new(file);
        BoneRig::read_le(&mut reader).map_err(BoneRigError::ParsingError)
    }

    pub fn from_memory(data: &[u8]) -> Result<Self, BoneRigError> {
        let mut reader = Cursor::new(data);
        BoneRig::read_le(&mut reader).map_err(BoneRigError::ParsingError)
    }

    pub fn pack_to_vec(&self) -> Result<Vec<u8>, BoneRigError> {
        let mut writer = Cursor::new(Vec::new());
        self.pack_internal(&mut writer)?;
        Ok(writer.into_inner())
    }

    pub fn pack_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), BoneRigError> {
        let file = fs::File::create(path).map_err(BoneRigError::IoError)?;
        let mut writer = BufWriter::new(file);
        self.pack_internal(&mut writer)?;
        Ok(())
    }

    fn pack_internal<W: Write + Seek>(&self, writer: &mut W) -> Result<(), BoneRigError> {
        self.write_le(writer)
            .map_err(|e| BoneRigError::SerializationError(e.to_string()))?;
        Ok(())
    }

    pub fn iter_poses(&self) -> impl Iterator<Item=PoseData<'_>> {
        (0..self.pose_bone_info.pose_entry_indices.len()).flat_map(move |i| self.get_pose(i as u32))
    }

    pub fn get_pose(&self, pose_index: u32) -> Option<PoseData<'_>> {


        let pose_index_loc = pose_index as usize;
        let offset = *self.pose_bone_info.pose_entry_indices.get(pose_index_loc)? as usize;
        let length = *self.pose_bone_info.pose_bones_counts.get(pose_index_loc)? as usize;
        let pose_bone_indices = self.pose_bone_info.pose_bone_indices.get(offset..offset + length)?;
        let pose_bones = self.pose_bone_info.pose_bones.get(offset..offset + length)?;

        let pose_name = self.pose_bone_info.pose_name_list.get(pose_index_loc)?;
        let bones = pose_bone_indices.iter().enumerate().flat_map(|(i, bone_index)| {
            let bone = self.bone_definitions.get(bone_index.clone() as usize)?;
            let pose_bone = pose_bones.get(i)?;
            Some((bone, pose_bone))
        }).collect::<Vec<_>>();

        Some(PoseData { pose_name: String::from_utf8(pose_name.to_vec()).ok()?, bones })
    }

    pub fn global_bone_matrix(&self, bone_index: usize) -> Option<Transform> {
        if bone_index > i32::MAX as usize {
            return None;
        }
        Some(self.bind_pose_inv_global_mats.get(bone_index)?.inverse()?.into())
    }

    fn compute_inv_global_matrices(&self) -> Vec<Matrix43> {
        self.compute_global_matrices().iter().map(|matrix| matrix.inverse()).flatten().collect::<Vec<_>>()
    }

    fn compute_global_matrices(&self) -> Vec<Matrix43> {
        use nalgebra::{Matrix4, Vector3, UnitQuaternion, Quaternion, Translation3};

        struct Bone{
            position: Vector3<f32>,
            rotation: UnitQuaternion<f32>,
            children: Vec<usize>,
        }

        let mut bones: Vec<Bone> = self
            .bone_definitions
            .iter()
            .enumerate()
            .map(|(i, bone_def)| {
                let transform = &self.bind_pose[i];

                let position = Vector3::new(transform.position.x, -transform.position.z, transform.position.y);
                let rot = UnitQuaternion::from_quaternion(
                    Quaternion::new(transform.rotation.w, -transform.rotation.x, transform.rotation.z, -transform.rotation.y)
                );

                let rotation = if i == 0 {
                    let root_rot = UnitQuaternion::from_euler_angles(-std::f32::consts::FRAC_PI_2, 0.0, 0.0);
                    rot * root_rot
                } else {
                    rot
                };

                // Gather children
                let children: Vec<usize> = self
                    .bone_definitions
                    .iter()
                    .enumerate()
                    .filter(|(_, child_def)| {
                        child_def.prev_bone_nr.is_some_and(|parent_id| parent_id == i)
                    })
                    .map(|(child_idx, _)| child_idx)
                    .collect();

                Bone{
                    position,
                    rotation,
                    children
                }

            })
            .collect();

        let mut matrices = vec![Matrix43::identity(); bones.len()];
        let mut stack = vec![(0, Matrix4::identity())];
        while let Some((bone_id, parent_bind_mat)) = stack.pop() {
            let bone = &mut bones[bone_id];
            let translation = Translation3::from(bone.position).to_homogeneous();
            let rotation = bone.rotation.to_homogeneous();
            let matrix: Matrix4<f32> = parent_bind_mat * (translation * rotation);

            let rotation_euler = UnitQuaternion::from_euler_angles(std::f32::consts::FRAC_PI_2, 0.0, 0.0).to_homogeneous();
            let global_matrix = matrix * rotation_euler;

            matrices[bone_id] = global_matrix.into();
            for &child_id in &bone.children {
                stack.push((child_id, matrix));
            }
        }
        matrices
    }

    //implement a set_pose function
}