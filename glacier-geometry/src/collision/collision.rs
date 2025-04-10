use std::io::{Seek, Write};
use binrw::{BinRead, binread, BinResult, BinWrite, BinWriterExt, Endian, binrw};
use crate::render_primitive::{PrimPropertyFlags};
use crate::utils::io::align_writer;
use crate::utils::math::{BoundingBox, Vector3};


#[binrw]
#[derive(Debug, PartialEq, Clone)]
#[br(import{ global_properties: PrimPropertyFlags })]
pub enum Collision {
    #[br(pre_assert(!global_properties.is_linked_object()))]
    Box(BoxColi),

    #[br(pre_assert(global_properties.is_linked_object()))]
    Bone(BoneColi),
}

#[binread]
#[derive(Debug, PartialEq, Clone)]
pub struct BoxColi {
    #[br(temp)]
    pub num_chunks: u16,

    pub tri_per_chunk: u16,

    #[br(
    parse_with = parse_hitbox,
    args(num_chunks)
    )]
    pub box_entries: Vec<BoundingBox<Vector3>>,
}

impl BinWrite for BoxColi {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, _args: Self::Args<'_>) -> BinResult<()> {
        u16::write_options(&(self.box_entries.len() as u16), writer, endian, ())?;
        writer.write_type(&self.tri_per_chunk, endian)?;
        for entry in self.box_entries.iter() {
            writer.write_type(&((entry.min.x * 255.0) as u8), endian)?;
            writer.write_type(&((entry.min.y * 255.0) as u8), endian)?;
            writer.write_type(&((entry.min.z * 255.0) as u8), endian)?;
            writer.write_type(&((entry.max.x * 255.0) as u8), endian)?;
            writer.write_type(&((entry.max.y * 255.0) as u8), endian)?;
            writer.write_type(&((entry.max.z * 255.0) as u8), endian)?;
        }
        align_writer(writer, 4)?;
        Ok(())
    }
}

#[binrw]
#[derive(Debug, PartialEq, Clone)]
pub struct BoneColi {
    pub total_size: u16,
    pub num_blocks: u16,
    pub total_chunks_align: u16,

    #[br(count = (total_size - 6))]
    pub data: Vec<u8>,
}

#[binrw::parser(reader, endian)]
fn parse_hitbox(object_count: u16) -> BinResult<Vec<BoundingBox<Vector3>>> {
    let hitboxes = (0..object_count).map(|_| -> BinResult<BoundingBox<Vector3>> {
        Ok(BoundingBox {
            min: Vector3 {
                x: u8::read_options(reader, endian, ())? as f32 / 255.0,
                y: u8::read_options(reader, endian, ())? as f32 / 255.0,
                z: u8::read_options(reader, endian, ())? as f32 / 255.0,
            },
            max: Vector3 {
                x: u8::read_options(reader, endian, ())? as f32 / 255.0,
                y: u8::read_options(reader, endian, ())? as f32 / 255.0,
                z: u8::read_options(reader, endian, ())? as f32 / 255.0,
            },
        })
    });
    Ok(hitboxes.into_iter().flatten().collect())
}
