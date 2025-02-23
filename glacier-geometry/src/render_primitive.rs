
use std::{fs, io};
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path};
use binrw::{BinRead, binread, BinReaderExt, BinResult, BinWrite, BinWriterExt, Endian, FilePtr64};
use binrw::io::SeekFrom;
use bitfield_struct::bitfield;
use byte_slice_cast::{AsByteSlice, AsSliceOf};
use itertools::{izip, Either, Itertools};
use num_traits::zero;
use crate::model::prim_mesh::PrimMesh;
use crate::model::prim_mesh_weighted::PrimMeshWeighted;
use crate::model::prim_object::ObjectPropertyFlags;
use crate::utils::math::{BoundingBox, Color, Vector2, Vector3, Vector4};
use wide::f32x4;
use crate::model::prim_mesh_linked::PrimMeshLinked;
use crate::utils::buffer::{IndexBuffer, Vertex, VertexWeights};

#[binread]
#[derive(Debug, PartialEq, Clone)]
#[brw(little)]
pub struct RenderPrimitive {
    #[br(parse_with = FilePtr64::parse)]
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
    pub fn parse(path: &Path) -> BinResult<RenderPrimitive> {
        let mut reader = Cursor::new(fs::read(path).unwrap());
        let prim: RenderPrimitive = reader.read_ne()?;
        Ok(prim)
    }

    pub fn parse_bytes<A : Read + Seek>(data: &mut A) -> BinResult<RenderPrimitive> {
        let prim = RenderPrimitive::read_le_args(data, ())?;
        Ok(prim)
    }

    pub fn write(&self, path: &Path) -> Result<(), binrw::Error> {
        let mut writer = Cursor::new(Vec::new());
        self.write_options(&mut writer, Endian::Little, ())?;
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
        self.data.objects.iter().filter(move |&obj| obj.prim_mesh().prim_object.lod_mask.clone() & lod_mask == lod_mask)
    }

    pub fn primitives_count(&self) -> usize {
        self.data.objects.len()
    }

    pub fn flags(&self) -> PrimPropertyFlags {
        self.data.property_flags
    }
}

impl BinWrite for RenderPrimitive {
    type Args<'a> = ();
    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, _args: Self::Args<'_>) -> BinResult<()> {
        let mut header_pointer: u64 = 0;
        let padd: u64 = 0;
        u64::write_options(&header_pointer, writer, endian, ())?;
        u64::write_options(&padd, writer, endian, ())?;


        PrimObjectHeader::write_options(&self.data, writer, endian, &mut header_pointer)?;

        writer.seek(SeekFrom::Start(0))?;
        u64::write_options(&header_pointer, writer, endian, ())?;

        Ok(())
    }
}

#[binread]
#[allow(dead_code, unused_variables)]
#[derive(Debug, PartialEq, Clone)]
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
    args(num_objects, property_flags,),
    )]
    pub objects: Vec<MeshObject>,

    #[br(temp)]
    pub min: Vector3,

    #[br(temp)]
    pub max: Vector3,
}

impl BinWrite for PrimObjectHeader {
    type Args<'a> = &'a mut u64;
    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let mut obj_offsets = (0..self.objects.len()).map(|_| 0).collect::<Vec<u32>>();

        for (i, object) in self.objects.iter().enumerate() {
            MeshObject::write_options(object, writer, endian, (&self.property_flags, &mut obj_offsets[i]))?;
        }


        let object_table_start_pos = writer.stream_position()?;
        for offset in obj_offsets {
            u32::write_options(&offset, writer, endian, ())?;
        }
        align_writer(writer, 16)?;

        let header_start_pos = writer.stream_position()?;
        *args = header_start_pos;
        writer.write_type(&self.prims, endian)?;
        writer.write_type(&self.property_flags, endian)?;
        writer.write_type(&self.bone_rig_resource_index.unwrap_or(0xFFFFFFFF), endian)?;
        writer.write_type(&u32::try_from(self.objects.len()).unwrap_or(0), endian)?;
        writer.write_type(&(object_table_start_pos as u32), endian)?;

        let bb : BoundingBox<Vector3> = self.objects.iter().map(|o| match o{
            MeshObject::Normal(o) => {o.calc_bb()}
            MeshObject::Weighted(o) => {o.prim_mesh.calc_bb()}
            MeshObject::Linked(o) => {o.prim_mesh.calc_bb()}
        }).sum();

        writer.write_type(&bb.min, endian)?;
        writer.write_type(&bb.max, endian)?;
        align_writer(writer, 8)?;

        Ok(())
    }
}

#[derive(BinRead, Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags))]
pub enum MeshObject {
    #[br(pre_assert(!global_properties.is_weighted_object() && !global_properties.is_linked_object()))]
    Normal(
        #[br(args(global_properties))]
        PrimMesh
    ),
    #[br(pre_assert(global_properties.is_weighted_object()))]
    Weighted(
        #[br(args(global_properties))]
        PrimMeshWeighted
    ),
    #[br(pre_assert(global_properties.is_linked_object()))]
    Linked(
        #[br(args(global_properties))]
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
        // Retrieve all attribute vectors
        let positions = self.get_positions();
        let normals = self.get_normals();
        let tangents = self.get_tangents();
        let bitangents = self.get_bitangents();
        let tex_coords = self.get_tex_coords();
        let weights = self.get_weights();
        let colors = self.get_colors();

        // Ensure all attribute vectors have the same length
        let len = positions.len();
        assert!(
            normals.len() == len
                && tangents.len() == len
                && bitangents.len() == len
                && tex_coords.len() == len,
            "All vertex attribute slices must have the same length"
        );

        // Create iterators for each attribute
        let pos_iter = positions.into_iter();
        let norm_iter = normals.into_iter();
        let tan_iter = tangents.into_iter();
        let bitan_iter = bitangents.into_iter();
        let tex_iter = tex_coords.into_iter();

        // Handle optional weights using Either to unify iterator types
        let weights_iter = match weights {
            Some(w) => Either::Left(w.into_iter().map(Some)),
            None => Either::Right(std::iter::repeat(None).take(len)),
        };

        // Similarly handle optional colors
        let colors_iter = match colors {
            Some(c) => Either::Left(c.into_iter().map(Some)),
            None => Either::Right(std::iter::repeat(None).take(len)),
        };

        // Use izip! to iterate over all attributes in parallel
        izip!(
            pos_iter,
            norm_iter,
            tan_iter,
            bitan_iter,
            tex_iter,
            weights_iter,
            colors_iter
        )
            .map(
                |(pos, norm, tan, bitan, tex, weight, color)| Vertex {
                    position: pos,
                    normal: norm,
                    tangent: tan,
                    bitangent: bitan,
                    uvs: tex,
                    weights: weight,
                    color,
                },
            )
            .collect()
    }

    pub fn get_positions(&self) -> Vec<Vector4> {
        let position_data = &self.prim_mesh().sub_mesh.buffers.position;
        if self.prim_mesh().prim_object.properties.has_highres_positions() {
            if let Ok(values ) = position_data.as_byte_slice().as_slice_of::<f32>() {
                values.chunks_exact(3).map(|v| {
                    Vector4{
                        x: v[0] * self.prim_mesh().pos_scale.x + self.prim_mesh().pos_bias.x,
                        y: v[1] * self.prim_mesh().pos_scale.y + self.prim_mesh().pos_bias.y,
                        z: v[2] * self.prim_mesh().pos_scale.z + self.prim_mesh().pos_bias.z,
                        w: 1.0  * self.prim_mesh().pos_scale.w + self.prim_mesh().pos_bias.w,
                    }
                }).collect()
            }else {
                vec![]
            }
        }else{
            if let Some(arr) = Self::dequantize_i16_to_f32(&self.prim_mesh().sub_mesh.buffers.position, self.prim_mesh().pos_scale.as_slice(), self.prim_mesh().pos_bias.as_slice()){
                arr.chunks_exact(4).map(|v| {
                    Vector4{
                        x: v[0],
                        y: v[1],
                        z: v[2],
                        w: v[3],
                    }
                }).collect()
            } else {
                vec![]
            }
        }
    }

    pub fn get_weights(&self) -> Option<Vec<VertexWeights>> {
        const FACTOR: f32 = 1.0 / u8::MAX as f32;
        let weights_data = self.prim_mesh().sub_mesh.buffers.weights.as_ref()?;
        Some(
            weights_data.chunks_exact(12)
                .map(|chunk| {
                    let weight = [
                        chunk[0] as f32 * FACTOR,
                        chunk[1] as f32 * FACTOR,
                        chunk[2] as f32 * FACTOR,
                        chunk[3] as f32 * FACTOR,

                        chunk[8] as f32 * FACTOR,
                        chunk[9] as f32 * FACTOR,
                    ];

                    let joint = [
                        chunk[4],
                        chunk[5],
                        chunk[6],
                        chunk[7],

                        chunk[10],
                        chunk[11],
                    ];

                    VertexWeights { weight, indices: joint }
                })
                .collect()
        )
    }

    pub fn get_normals(&self) -> Vec<Vector4> {
        self.get_ntb(0)
    }

    pub fn get_tangents(&self) -> Vec<Vector4> {
        self.get_ntb(4)
    }

    pub fn get_bitangents(&self) -> Vec<Vector4> {
        self.get_ntb(8)
    }

    pub fn get_tex_coords(&self) -> Vec<Vec<Vector2>> {
        let prim_mesh = self.prim_mesh();
        let sub_mesh = &prim_mesh.sub_mesh;
        let ntb_data = &sub_mesh.buffers.main;
        let num_uvs = sub_mesh.num_uv_channels;
        let uv_scale_bias = prim_mesh.tex_scale_bias;

        let ntb_stride = (12 + (num_uvs * 4) as usize) / 2;
        const MAX: f32 = i16::MAX as f32;

        let mut maps = vec![];

        if let Some(values) = ntb_data.as_byte_slice().as_slice_of::<i16>().ok() {
            let num_vertices = sub_mesh.num_vertices as usize;

            maps = vec![vec![Vector2::default(); num_vertices]; num_uvs as usize];

            for (vertex_index, chunk) in values.chunks_exact(ntb_stride).enumerate() {
                let uv_start = 6;
                for uv_channel in 0..num_uvs as usize {
                    let offset: usize = (uv_start + uv_channel * 2) as usize;
                    let u = (chunk[offset] as f32 * uv_scale_bias.x / MAX) + uv_scale_bias.z;
                    let v = (chunk[offset + 1] as f32 * uv_scale_bias.y / MAX) + uv_scale_bias.w;
                    maps[uv_channel][vertex_index] = Vector2 { x: u, y: v };
                }
            }
        }
        maps
    }

    pub fn get_colors(&self) -> Option<Vec<Color>> {
        let color_data = self.prim_mesh().sub_mesh.buffers.colors.as_ref()?;
        Some(
            color_data.chunks_exact(4)
                .map(|chunk| {
                    Color {
                        r: chunk[0],
                        g: chunk[1],
                        b: chunk[2],
                        a: chunk[3],
                    }
                })
                .collect()
        )
    }

    fn get_ntb(&self, offset: usize) -> Vec<Vector4> {
        let ntb_data = &self.prim_mesh().sub_mesh.buffers.main;

        let ntb_stride = 12 + (self.prim_mesh().sub_mesh.num_uv_channels * 4) as usize;

        let scale = 2.0;
        let bias = -1.0;
        let factor: f32 = scale / u8::MAX as f32;

        ntb_data.chunks_exact(ntb_stride).map(|ntb_uv| {
            let vec4: Vec<_> = ntb_uv.iter().skip(offset).take(4).collect();
            Vector4{
                x: (*vec4[0] as f32 * factor) + bias,
                y: (*vec4[1] as f32 * factor) + bias,
                z: (*vec4[2] as f32* factor) + bias,
                w: (*vec4[3] as f32 * factor) + bias,
            }
        }).collect()
    }

    fn dequantize_i16_to_f32(input: &[u8], scale: [f32; 4], bias: [f32; 4]) -> Option<Vec<f32>> {

        let values = input.as_byte_slice().as_slice_of::<i16>().ok()?;
        if values.len() % 4 != 0 {
            return None;
        }

        let count = values.len() / 4;
        let mut output = vec![0.0f32; values.len()];

        let max_val = i16::MAX as f32;

        let scale_vec = f32x4::from(scale);
        let bias_vec = f32x4::from(bias);
        let reciprocal_max = f32x4::splat(1.0 / max_val);

        for i in 0..count {
            let start = i * 4;

            let chunk = &values[start..start+4];
            let mut f32_vals = [0.0f32; 4];
            for (j, &val) in chunk.iter().enumerate() {
                f32_vals[j] = val as f32;
            }

            let vec_f32x4 = f32x4::from(f32_vals);
            let result = (vec_f32x4 * reciprocal_max) * scale_vec + bias_vec;

            let out_arr = result.to_array();
            output[start..start+4].copy_from_slice(&out_arr);
        }
        Some(output)
    }
}

impl BinWrite for MeshObject {
    type Args<'a> = (&'a PrimPropertyFlags, &'a mut u32);

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
fn parse_objects(object_count: u32, global_properties: PrimPropertyFlags) -> BinResult<Vec<MeshObject>> {
    let table_offset = u32::read_options(reader, endian, ())?;
    let saved_pos = reader.stream_position()?;
    reader.seek(SeekFrom::Start(table_offset as u64))?;

    let offset_table = (0..object_count).map(|_| {
        u32::read_options(reader, endian, ())
    });

    let mut objects = vec![];
    for offset in offset_table.flatten().collect::<Vec<_>>() {
        reader.seek(SeekFrom::Start(offset as u64))?;
        objects.push(MeshObject::read_options(reader, endian, (global_properties, ))?);
    }

    reader.seek(SeekFrom::Start(saved_pos))?;
    Ok(objects)
}

pub fn align_writer<W: Write + Seek>(writer: &mut W, num: usize) -> Result<(), io::Error> {
    let padding = (num - (writer.stream_position()? as usize % num)) % num;
    writer.write_all(vec![0; padding].as_slice())?;
    Ok(())
}