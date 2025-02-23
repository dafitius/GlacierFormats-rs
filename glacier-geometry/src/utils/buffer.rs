
use std::io::{Read, Seek, Write};
use std::marker::PhantomData;
use bincode::Encode;
use binrw::{BinRead, BinResult, BinWrite, BinWriterExt, Endian};
use num_traits::Bounded;

use crate::model::prim_mesh::PrimMesh;
use crate::model::prim_object::ObjectPropertyFlags;
use crate::render_primitive::PrimPropertyFlags;
use crate::utils::math::{Color, Vector2, Vector4};

pub type VertexPos = Vector4;
pub type VertexColor = Color;

#[derive(Debug, PartialEq, Clone, Encode)]
pub struct VertexMain
{
    pub normal: Vector4,
    pub tangent: Vector4,
    pub bitangent: Vector4,
    pub uvs: Vec<Vector2>,
}

pub struct Vertex {
    pub position: Vector4,
    pub normal: Vector4,
    pub tangent: Vector4,
    pub bitangent: Vector4,
    pub uvs: Vec<Vector2>,
    pub weights: Option<VertexWeights>,
    pub color: Option<Color>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VertexWeights
{
    pub weight: [f32; 6],
    pub indices: [u8; 6],
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct VertexBuffers {
    pub position: Vec<u8>,
    pub weights: Option<Vec<u8>>,
    pub main: Vec<u8>,
    pub colors: Option<Vec<u8>>,
}

pub type IndexBuffer = Vec<u16>;

#[binrw::parser(reader, endian)]
pub fn parse_vertices(
    num_vertices: u32,
    has_highres: bool,
    is_weighted: bool,
    num_uv_channels: u8,
    has_const_color: bool,
    has_global_const_color: bool
) -> BinResult<VertexBuffers> {

    let position_stride = if has_highres {
        size_of::<f32>() * 3 // raw float x, y, z, 0
    } else {
        size_of::<i16>() * 4 // quantized x, y, z, a
    };
    let position_size = position_stride * num_vertices as usize;
    let mut position = vec![0u8; position_size];
    reader.read_exact(&mut position)?;

    let weights = if is_weighted {
        let weight_stride = 12 * size_of::<u8>();
        let weight_size = weight_stride * num_vertices as usize;

        let mut weight_buffer = vec![0u8; weight_size];
        reader.read_exact(&mut weight_buffer)?;
        Some(weight_buffer)
    } else {
        None
    };

    // Read main buffer
    // main includes normals/tangents/bitangents (12 bytes per vertex) and UV coordinates
    let normals_stride = 12 * size_of::<u8>(); // quantized x,y,z,w * 3
    let uv_stride = (2 * size_of::<i16>()) * num_uv_channels as usize;
    let main_stride = normals_stride + uv_stride;
    let main_size = main_stride * num_vertices as usize;

    let mut main = vec![0u8; main_size];
    reader.read_exact(&mut main)?;

    let has_color_buffer = (is_weighted || !has_const_color) && !has_global_const_color;

    let colors = if has_color_buffer {
        let color_stride = 4 * size_of::<u8>();
        let color_size = color_stride * num_vertices as usize;

        let mut color_buffer = vec![0u8; color_size];
        reader.read_exact(&mut color_buffer)?;
        Some(color_buffer)
    } else {
        None
    };

    Ok(VertexBuffers {
        position,
        weights,
        main,
        colors,
    })
}

impl BinWrite for VertexBuffers {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        // Write the position buffer
        writer.write_all(&self.position)?;

        // Write the weights buffer if it exists
        if let Some(weights) = &self.weights {
            writer.write_all(weights)?;
        }

        // Write the main buffer
        writer.write_all(&self.main)?;

        // Write the colors buffer if it exists
        if let Some(colors) = &self.colors {
            writer.write_all(colors)?;
        }

        Ok(())
    }
}