use std::io::{Seek, SeekFrom, Write};
use binrw::{binread, BinResult, BinWrite, BinWriterExt, Endian, FilePtr32};
use byte_slice_cast::{AsByteSlice, AsSliceOf};
use itertools::{izip, Either};
use wide::f32x4;
use crate::mesh::prim_sub_mesh::PrimSubMesh;
use crate::model::prim_object::PrimObject;
use crate::render_primitive::{align_writer, PrimPropertyFlags};
use crate::utils::buffer::{IndexBuffer, Vertex, VertexWeights};
use crate::utils::math::{BoundingBox, Color, Vector2, Vector3, Vector4};

#[binread]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
#[br(import(global_properties: PrimPropertyFlags))]
pub struct PrimMesh
{
    pub prim_object: PrimObject,

    #[br(temp)]
    sub_mesh_table_offset: u32,

    pub pos_scale: Vector4,

    pub pos_bias: Vector4,

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
    cloth_id
    }
    })]
    pub sub_mesh: PrimSubMesh,
}

impl BinWrite for PrimMesh {
    type Args<'a> = (&'a PrimPropertyFlags, &'a mut u32);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        let mut sub_mesh_ptr: u32 = 0;
        PrimSubMesh::write_options(&self.sub_mesh, writer, endian, (self, args.0, &mut sub_mesh_ptr))?;

        *args.1 = writer.stream_position()? as u32;
        PrimObject::write_options(&self.prim_object, writer, endian, (self.calc_bb(),))?;
        writer.write_type(&sub_mesh_ptr, endian)?; //sub_mesh_offset
        if args.0.has_highres_positions() {
            writer.write_type(&Vector4{ x: 1.0, y: 1.0, z: 1.0, w: 1.0 },endian)?;
            writer.write_type(&Vector4{ x: 0.0, y: 0.0, z: 0.0, w: 0.0 },endian)?;
        }else{
            writer.write_type(&self.pos_scale, endian)?;
            writer.write_type(&self.pos_bias, endian)?;
        }
        writer.write_type(&self.tex_scale_bias, endian)?;
        writer.write_type(&self.cloth_id, endian)?;
        align_writer(writer, 16)?;

        Ok(())
    }
}

impl PrimMesh {

    pub fn calc_bb(&self) -> BoundingBox<Vector3> {
        let mut min_bb = Vector3 { x: f32::INFINITY, y: f32::INFINITY, z: f32::INFINITY };
        let mut max_bb = Vector3 { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY, z: f32::NEG_INFINITY };

        for pos in self.get_positions() {
            min_bb.x = min_bb.x.min(pos.x);
            max_bb.x = max_bb.x.max(pos.x);

            min_bb.y = min_bb.y.min(pos.y);
            max_bb.y = max_bb.y.max(pos.y);

            min_bb.z = min_bb.z.min(pos.z);
            max_bb.z = max_bb.z.max(pos.z);
        }

        BoundingBox { min: min_bb, max: max_bb }
    }

    pub fn calc_uv_bb(&self) -> BoundingBox<Vector2> {
        let mut min_bb = Vector2 { x: f32::INFINITY, y: f32::INFINITY };
        let mut max_bb = Vector2 { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY };

        for layer in self.get_tex_coords() {
            for pos in layer.iter() {
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

    pub fn get_indices(&self) -> &IndexBuffer {
        &self.sub_mesh.indices
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
        let position_data = &self.sub_mesh.buffers.position;
        if self.prim_object.properties.has_highres_positions() {
            if let Ok(values ) = position_data.as_byte_slice().as_slice_of::<f32>() {
                values.chunks_exact(3).map(|v| {
                    Vector4{
                        x: v[0] * self.pos_scale.x + self.pos_bias.x,
                        y: v[1] * self.pos_scale.y + self.pos_bias.y,
                        z: v[2] * self.pos_scale.z + self.pos_bias.z,
                        w: 1.0  * self.pos_scale.w + self.pos_bias.w,
                    }
                }).collect()
            }else {
                vec![]
            }
        }else{
            if let Some(arr) = Self::dequantize_i16_to_f32(&self.sub_mesh.buffers.position, self.pos_scale.as_slice(), self.pos_bias.as_slice()){
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
        let weights_data = self.sub_mesh.buffers.weights.as_ref()?;
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
        let prim_mesh = self;
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
        let color_data = self.sub_mesh.buffers.colors.as_ref()?;
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
        let ntb_data = &self.sub_mesh.buffers.main;

        let ntb_stride = 12 + (self.sub_mesh.num_uv_channels * 4) as usize;

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
