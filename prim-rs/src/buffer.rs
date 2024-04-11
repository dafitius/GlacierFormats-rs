
use std::io::{Read, Seek, Write};
use std::marker::PhantomData;
use base64::Engine;
use base64::engine::general_purpose;
use bincode::Encode;
use binrw::{BinRead, BinResult, BinWrite, BinWriterExt, Endian};
use num_traits::Bounded;

use crate::math::{Color, Vector2, Vector4};
use crate::render_primitive::{PrimPropertyFlags};
use crate::prim_object::ObjectPropertyFlags;

#[cfg(feature = "serde")]
use serde::{Serialize, Serializer};
#[cfg(feature = "serde")]
use serde::ser::SerializeStruct;
use crate::prim_mesh::PrimMesh;


pub type VertexPos = Vector4;
pub type VertexColor = Color;

#[derive(Debug, PartialEq, Encode)]
pub struct VertexMain
{
    pub normal: Vector4,
    pub tangent: Vector4,
    pub bitangent: Vector4,
    pub uvs: Vec<Vector2>,
}

#[derive(Debug, PartialEq)]
pub struct VertexWeights
{
    pub weight: (Vector4, Vector2),
    pub joint: (Vector4, Vector2),
}

#[derive(Debug, PartialEq)]
pub struct VertexBuffers {
    pub position: Vec<VertexPos>,
    pub weights: Option<Vec<VertexWeights>>,
    pub main: Vec<VertexMain>,
    pub colors: Option<Vec<VertexColor>>,
}

impl VertexBuffers {
    pub fn num_vertices(&self) -> u32 {
        self.position.len() as u32
    }

    pub fn num_uv_channels(&self) -> u8 {
        if self.num_vertices() > 0 {
            self.main[0].uvs.len() as u8
        } else { 0 }
    }
}

impl BinWrite for VertexBuffers {
    type Args<'a> = (&'a PrimMesh, &'a ObjectPropertyFlags, &'a PrimPropertyFlags);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        for vertex in self.position.iter() {
            if args.2.has_highres_positions() {
                writer.write_type(&vertex.x, endian)?;
                writer.write_type(&vertex.y, endian)?;
                writer.write_type(&vertex.z, endian)?;
            } else {
                QuantizedVector4::<i16>::write_options(&QuantizedVector4::new(vertex), writer, endian, (args.0.calc_pos_scale(), args.0.calc_pos_bias()))?;
            }
        }

        if let Some(weights) = &self.weights {
            for vertex in weights {
                u8::write_options(&((vertex.weight.0.x * 255.0) as u8), writer, endian, ())?;
                u8::write_options(&((vertex.weight.0.y * 255.0) as u8), writer, endian, ())?;
                u8::write_options(&((vertex.weight.0.z * 255.0) as u8), writer, endian, ())?;
                u8::write_options(&((vertex.weight.0.w * 255.0) as u8), writer, endian, ())?;

                u8::write_options(&(vertex.joint.0.x as u8), writer, endian, ())?;
                u8::write_options(&(vertex.joint.0.y as u8), writer, endian, ())?;
                u8::write_options(&(vertex.joint.0.z as u8), writer, endian, ())?;
                u8::write_options(&(vertex.joint.0.w as u8), writer, endian, ())?;

                u8::write_options(&((vertex.weight.1.x * 255.0) as u8), writer, endian, ())?;
                u8::write_options(&((vertex.weight.1.y * 255.0) as u8), writer, endian, ())?;

                u8::write_options(&(vertex.joint.1.x as u8), writer, endian, ())?;
                u8::write_options(&(vertex.joint.1.y as u8), writer, endian, ())?;
            }
        }

        for vertex in self.main.iter() {
            let ntb_args = (Vector4::from_float(2.0), Vector4::from_float(-1.0));

            QuantizedVector4::<u8>::write_options(&QuantizedVector4::new(&vertex.normal), writer, endian, ntb_args)?;
            QuantizedVector4::<u8>::write_options(&QuantizedVector4::new(&vertex.tangent), writer, endian, ntb_args)?;
            QuantizedVector4::<u8>::write_options(&QuantizedVector4::new(&vertex.bitangent), writer, endian, ntb_args)?;
            for uv in vertex.uvs.iter() {
                QuantizedVector2::<i16>::write_options(&QuantizedVector2::new(uv), writer, endian, args.0.calc_uv_scale_bias())?;
            }
        }

        if let Some(colors) = &self.colors {
            for vertex in colors {
                writer.write_type(vertex, endian)?;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "serde")]
impl Serialize for VertexBuffers {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut buffers = serializer.serialize_struct("VertexBuffers", 3)?;

        let pos_bytes = bincode::encode_to_vec(&self.position, bincode::config::standard()).map_err(serde::ser::Error::custom)?;
        let pos_encoded = general_purpose::STANDARD.encode(pos_bytes);

        buffers.serialize_field("position", &pos_encoded)?;

        let main_bytes = bincode::encode_to_vec(&self.main, bincode::config::standard()).map_err(serde::ser::Error::custom)?;
        let main_encoded = general_purpose::STANDARD.encode(main_bytes);

        buffers.serialize_field("main", &main_encoded)?;
        buffers.end()
    }
}

pub type IndexBuffer = Vec<u16>;


#[derive(Debug, Default)]
pub struct QuantizedVector2<T> where
    T: Bounded + Into<f32> {
    phantom_data: PhantomData<T>,
    pub x: f32,
    pub y: f32,
}

impl<T> From<QuantizedVector2<T>> for Vector2
    where
        T: Ord + BinRead + Into<f32> + Copy + Bounded,
        for<'a> <T as BinRead>::Args<'a>: Default,
{
    fn from(quantized_vector: QuantizedVector2<T>) -> Self {
        let x = quantized_vector.x;
        let y = quantized_vector.y;
        Vector2 { x, y }
    }
}

#[binrw::parser(reader)]
fn dequantize<T: binrw::BinRead + Bounded + Into<f32>>(scale: f32, bias: f32) -> BinResult<f32> where for<'a> <T as BinRead>::Args<'a>: Default {
    let val = T::read_le(reader)?;
    Ok((val.into() * scale / T::max_value().into()) + bias)
}

#[binrw::writer(writer, endian)]
fn quantize<T>(obj: &f32, scale: f32, bias: f32) -> BinResult<()>
    where
        T: binrw::BinWrite + Bounded + Into<f32> + TryFrom<isize>,
        for<'a> <T as BinWrite>::Args<'a>: Default
{
    let val_int: isize = (T::max_value().into() * (obj - bias) / scale).round() as isize;
    let val_t : T = val_int.try_into().map_err(|_| binrw::Error::AssertFail {pos: writer.stream_position().unwrap_or(0), message: format!("Can't convert {} to requested type", val_int)})?;
    writer.write_type(&val_t, endian)?;
    Ok(())
}

impl<T> BinRead for QuantizedVector2<T> where
    T: BinRead + Bounded + Into<f32>,
    for<'a> <T as BinRead>::Args<'a>: Default {
    type Args<'a> = Vector4;

    fn read_options<R: Read + Seek>(reader: &mut R, endian: Endian, args: Self::Args<'_>) -> BinResult<Self> {
        Ok(Self {
            phantom_data: Default::default(),
            x: dequantize::<T, R>(reader, endian, (args.x, args.z))?,
            y: dequantize::<T, R>(reader, endian, (args.y, args.w))?,
        })
    }
}

impl<T> BinWrite for QuantizedVector2<T> where
    T: BinWrite + Bounded + Into<f32> + TryFrom<isize>,
    for<'a> <T as BinWrite>::Args<'a>: Default {
    type Args<'a> = Vector4;

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        quantize::<T, W>(&self.x, writer, endian, (args.x, args.z))?;
        quantize::<T, W>(&self.y, writer, endian, (args.y, args.w))?;
        Ok(())
    }
}

impl<T> QuantizedVector2<T> where
    T: Ord + Into<f32> + Copy + Bounded {
    pub fn new(vector: &Vector2) -> Self {
        Self { phantom_data: Default::default(), x: vector.x, y: vector.y }
    }
}


#[derive(Debug, Default)]
pub struct QuantizedVector4<T> where
    T: Bounded + Into<f32> {
    phantom_data: PhantomData<T>,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl<T> BinRead for QuantizedVector4<T> where
    T: BinRead + Bounded + Into<f32>,
    for<'a> <T as BinRead>::Args<'a>: Default {
    type Args<'a> = (Vector4, Vector4);

    fn read_options<R: Read + Seek>(reader: &mut R, endian: Endian, args: Self::Args<'_>) -> BinResult<Self> {
        Ok(Self {
            phantom_data: Default::default(),
            x: dequantize::<T, R>(reader, endian, (args.0.x, args.1.x))?,
            y: dequantize::<T, R>(reader, endian, (args.0.y, args.1.y))?,
            z: dequantize::<T, R>(reader, endian, (args.0.z, args.1.z))?,
            w: dequantize::<T, R>(reader, endian, (args.0.w, args.1.w))?,
        })
    }
}

impl<T> BinWrite for QuantizedVector4<T> where
    T: BinWrite + Bounded + Into<f32> + TryFrom<isize>,
    for<'a> <T as BinWrite>::Args<'a>: Default {
    type Args<'a> = (Vector4, Vector4);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {

        quantize::<T, W>(&self.x, writer, endian, (args.0.x, args.1.x))?;
        quantize::<T, W>(&self.y, writer, endian, (args.0.y, args.1.y))?;
        quantize::<T, W>(&self.z, writer, endian, (args.0.z, args.1.z))?;
        quantize::<T, W>(&self.w, writer, endian, (args.0.w, args.1.w))?;
        Ok(())
    }
}

impl<T> From<QuantizedVector4<T>> for Vector4
    where
        T: Ord + BinRead + Into<f32> + Copy + Bounded,
        for<'a> <T as BinRead>::Args<'a>: Default,
{
    fn from(quantized_vector: QuantizedVector4<T>) -> Self {
        let x = quantized_vector.x;
        let y = quantized_vector.y;
        let z = quantized_vector.z;
        let w = quantized_vector.w;
        Vector4 { x, y, z, w }
    }
}

impl<T> QuantizedVector4<T> where
    T: Ord + Into<f32> + Copy + Bounded {
    pub fn new(vector: &Vector4) -> Self {
        Self { phantom_data: Default::default(), x: vector.x, y: vector.y, z: vector.z, w: vector.w }
    }
}

#[binrw::parser(reader, endian)]
pub fn parse_vertices(num_vertices: u32,
                  has_highres: bool,
                  is_weighted: bool,
                  num_uv_channels: u8,
                  has_const_color: bool,
                  has_global_const_color: bool,
                  pos_scale: Vector4,
                  pos_bias: Vector4,
                  tex_scale_bias: Vector4) -> BinResult<VertexBuffers> {
    let positions: Vec<_> = (0..num_vertices).map(|_| -> BinResult<VertexPos> {
        if has_highres {
            Ok(Vector4 {
                x: f32::read_options(reader, endian, ())? * pos_scale.x + pos_bias.x,
                y: f32::read_options(reader, endian, ())? * pos_scale.y + pos_bias.y,
                z: f32::read_options(reader, endian, ())? * pos_scale.z + pos_bias.z,
                w: 1.0 * pos_scale.w + pos_bias.w,
            }
            )
        } else {
            let quan = QuantizedVector4::<i16>::read_options(reader, endian, (pos_scale, pos_bias))?;
            Ok(quan.into())
        }
    }).collect();

    let weights_joints: Vec<_> = match is_weighted {
        true => {
            (0..num_vertices).map(|_| -> BinResult<VertexWeights> {
                let mut weight: (Vector4, Vector2) = (Vector4::default(), Vector2::default());
                let mut joint: (Vector4, Vector2) = (Vector4::default(), Vector2::default());
                weight.0.x = u8::read_options(reader, endian, ())? as f32 / 255.0;
                weight.0.y = u8::read_options(reader, endian, ())? as f32 / 255.0;
                weight.0.z = u8::read_options(reader, endian, ())? as f32 / 255.0;
                weight.0.w = u8::read_options(reader, endian, ())? as f32 / 255.0;

                joint.0.x = u8::read_options(reader, endian, ())? as f32;
                joint.0.y = u8::read_options(reader, endian, ())? as f32;
                joint.0.z = u8::read_options(reader, endian, ())? as f32;
                joint.0.w = u8::read_options(reader, endian, ())? as f32;

                weight.1.x = u8::read_options(reader, endian, ())? as f32 / 255.0;
                weight.1.y = u8::read_options(reader, endian, ())? as f32 / 255.0;

                joint.1.x = u8::read_options(reader, endian, ())? as f32;
                joint.1.y = u8::read_options(reader, endian, ())? as f32;

                Ok(VertexWeights {
                    weight,
                    joint,
                })
            }).collect()
        }
        false => { vec![] }
    };

    let ntb_uv: Vec<_> = (0..num_vertices).map(|_| -> BinResult<VertexMain> {
        let mut uvs = vec![];

        let args = (Vector4::from_float(2.0), Vector4::from_float(-1.0));
        let normal = QuantizedVector4::<u8>::read_options(reader, endian, args)?;
        let tangent = QuantizedVector4::<u8>::read_options(reader, endian, args)?;
        let bitangent = QuantizedVector4::<u8>::read_options(reader, endian, args)?;

        for _ in 0..num_uv_channels {
            uvs.push(
                QuantizedVector2::<i16>::read_options(reader, endian, tex_scale_bias)?
            )
        }
        Ok(VertexMain {
            normal: normal.into(),
            tangent: tangent.into(),
            bitangent: bitangent.into(),
            uvs: uvs.into_iter().map(|v| v.into()).collect(),
        })
    }).collect();

    let has_color_buffer = (is_weighted || !has_const_color) && !has_global_const_color;

    let colors: Vec<_> = match has_color_buffer {
        true => {
            (0..num_vertices).map(|_| -> BinResult<VertexColor> {
                Ok(Color {
                    r: u8::read_options(reader, endian, ())?,
                    g: u8::read_options(reader, endian, ())?,
                    b: u8::read_options(reader, endian, ())?,
                    a: u8::read_options(reader, endian, ())?
                })
            }).collect()
        }
        false => { vec![] }
    };


    Ok(VertexBuffers {
        position: positions.into_iter().flatten().collect(),
        weights: match is_weighted {
            true => { Some(weights_joints.into_iter().flatten().collect()) }
            false => { None }
        },
        main: ntb_uv.into_iter().flatten().collect(),
        colors: match has_color_buffer {
            true => { Some(colors.into_iter().flatten().collect()) }
            false => { None }
        },
    })
}