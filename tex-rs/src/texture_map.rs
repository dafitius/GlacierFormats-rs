use std::io;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use binrw::{BinRead, binread, BinReaderExt, BinResult, binrw, BinWrite, BinWriterExt, Endian};
use binrw::helpers::until_eof;
use serde::{Deserialize, Serialize};
use crate::enums::*;
use crate::WoaVersion;

const MAX_MIP_LEVELS: usize = 0xE;

#[derive(Debug, thiserror::Error)]
pub enum TextureMapError {
    #[error("Io error")]
    IoError(#[from] io::Error),

    #[error("Parsing error")]
    ParsingError(#[from] binrw::Error),

    #[error("Failed on {0}")]
    UnknownError(String),
}

pub struct DynamicTextureMapArgs {
    pub(crate) data_size: u32,

    pub(crate) atlas_data_size: u32,

    pub(crate) text_scale: u8,
    pub(crate) text_mip_levels: u8,
}

pub trait TextureMapHeaderImpl {
    fn get_text_scale(&self) -> usize;
    fn size() -> usize;
    fn text_data_size(&self) -> usize;
    fn has_atlas(&self) -> bool;
    fn texd_mip_levels(&self) -> usize;
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[br(assert(
    num_textures == 1 && num_textures != 6, "Looks like you tried to export a cubemap texture, those are not supported yet"
))]
#[bw(import(args: DynamicTextureMapArgs))]
pub struct TextureMapHeaderV1 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    pub(crate) type_: TextureType,

    pub(crate) texd_identifier: u32,

    #[br(temp)]
    #[bw(calc(args.data_size - 8))]
    data_size: u32,
    pub(crate) flags: RenderResourceMiscFlags,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) format: RenderFormat,
    pub(crate) num_mip_levels: u8,
    pub(crate) default_mip_level: u8,
    pub(crate) interpret_as: InterpretAs,
    pub(crate) dimensions: Dimensions,
    #[br(temp)]
    #[bw(calc(0))]
    mips_interpolation_deprecated: u16,

    pub(crate) mip_sizes: [u32; MAX_MIP_LEVELS],
    #[br(temp)]
    #[bw(calc(args.atlas_data_size))]
    atlas_data_size: u32,
    #[br(temp)]
    #[bw(calc(0x54))]
    atlas_data_offset: u32,

    //additional properties
    #[br(calc = atlas_data_size > 0)]
    #[bw(ignore)]
    pub(crate) has_atlas: bool,
}

impl TextureMapHeaderImpl for TextureMapHeaderV1 {
    fn get_text_scale(&self) -> usize
    {
        let texd_mips = self.num_mip_levels as usize;

        if texd_mips == 1 {
            return 0;
        }

        if self.interpret_as == InterpretAs::Billboard {
            return 0;
        }

        let area = self.width as usize * self.height as usize;
        ((area as f32).log2() * 0.5 - 6.5).floor() as usize
    }

    fn size() -> usize {
        92
    }

    fn text_data_size(&self) -> usize {
        let text_mip_levels = self.num_mip_levels as usize - self.get_text_scale();
        let blocks_to_skip = self.num_mip_levels as usize - text_mip_levels;
        let last_mip_size = self.mip_sizes[(self.num_mip_levels - 1) as usize] as usize;
        if blocks_to_skip == 0 {
            return last_mip_size;
        }
        let texd_mip_size = self.mip_sizes.get(blocks_to_skip - 1).unwrap_or(&0);
        last_mip_size - *texd_mip_size as usize
    }

    fn has_atlas(&self) -> bool {
        self.has_atlas
    }

    fn texd_mip_levels(&self) -> usize {
        self.num_mip_levels as usize
    }
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[br(assert(mip_sizes == compressed_mip_sizes))]
#[br(assert(num_textures == 1))]
#[bw(import(args: DynamicTextureMapArgs))]
pub struct TextureMapHeaderV2 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    pub(crate) type_: TextureType,

    #[br(temp)]
    #[bw(calc(args.data_size))]
    data_size: u32,
    pub(crate) flags: RenderResourceMiscFlags,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) format: RenderFormat,
    pub(crate) num_mip_levels: u8,
    pub(crate) default_mip_level: u8,
    pub(crate) texd_identifier: u32,
    pub(crate) mip_sizes: [u32; MAX_MIP_LEVELS],
    pub(crate) compressed_mip_sizes: [u32; MAX_MIP_LEVELS],
    #[br(temp)]
    #[bw(calc(args.atlas_data_size))]
    atlas_data_size: u32,
    #[br(temp)]
    #[bw(calc(0x90))]
    atlas_data_offset: u32,

    //additional properties
    #[br(calc = atlas_data_size > 0)]
    #[bw(ignore)]
    pub(crate) has_atlas: bool,
}

impl TextureMapHeaderImpl for TextureMapHeaderV2 {
    fn get_text_scale(&self) -> usize
    {
        let texd_mips = self.num_mip_levels as usize;
        if texd_mips == 1 {
            return 0;
        }

        if self.type_ == TextureType::Billboard {
            return 0;
        }

        if self.format == RenderFormat::DXT1 && (self.width as usize * self.height as usize) == 16 {
            return 1;
        }

        if self.texd_identifier != 16384 {
            return 0;
        }

        let area = self.width as usize * self.height as usize;
        ((area as f32).log2() * 0.5 - 6.5).floor() as usize
    }

    fn size() -> usize {
        144
    }

    fn text_data_size(&self) -> usize {
        let text_mip_levels = self.num_mip_levels as usize - self.get_text_scale();
        let blocks_to_skip = self.num_mip_levels as usize - text_mip_levels;
        let last_mip_size = self.compressed_mip_sizes[(self.num_mip_levels - 1) as usize] as usize;
        if blocks_to_skip == 0 {
            return last_mip_size;
        }
        let texd_mip_size = self.compressed_mip_sizes.get(blocks_to_skip - 1).unwrap_or(&0);
        last_mip_size - *texd_mip_size as usize
    }

    fn has_atlas(&self) -> bool {
        self.has_atlas
    }

    fn texd_mip_levels(&self) -> usize {
        self.num_mip_levels as usize
    }
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[br(assert(text_scaling_width == num_mip_levels - text_mip_levels))]
#[br(assert(text_scaling_height == num_mip_levels - text_mip_levels))]
#[br(assert(num_textures == 1))]
#[bw(import(args: DynamicTextureMapArgs))]
pub struct TextureMapHeaderV3 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    pub(crate) type_: TextureType,

    #[br(temp)]
    #[bw(calc(args.data_size))]
    data_size: u32,
    pub(crate) flags: RenderResourceMiscFlags,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) format: RenderFormat,
    pub(crate) num_mip_levels: u8,
    pub(crate) default_mip_level: u8,
    pub(crate) interpret_as: InterpretAs,
    pub(crate) dimensions: Dimensions,

    #[br(temp)]
    #[bw(calc(0))]
    mips_interpolation_deprecated: u16,
    pub(crate) mip_sizes: [u32; MAX_MIP_LEVELS],
    pub(crate) compressed_mip_sizes: [u32; MAX_MIP_LEVELS],
    #[br(temp)]
    #[bw(calc(args.atlas_data_size))]
    atlas_data_size: u32,
    #[br(temp)]
    #[bw(calc(0x98))]
    atlas_data_offset: u32,
    #[br(temp)]
    #[bw(calc(0xFF))]
    text_scaling_data1: u8,
    #[br(temp)]
    #[bw(calc(args.text_scale))]
    text_scaling_width: u8,
    #[br(temp)]
    #[bw(calc(args.text_scale))]
    text_scaling_height: u8,

    #[br(temp)]
    #[bw(calc(args.text_mip_levels))]
    #[brw(pad_after = 0x4)]
    text_mip_levels: u8,

    //additional properties
    #[br(calc = atlas_data_size > 0)]
    #[bw(ignore)]
    pub(crate) has_atlas: bool,
}

impl TextureMapHeaderImpl for TextureMapHeaderV3 {
    fn get_text_scale(&self) -> usize {
        let texd_mips = self.num_mip_levels as usize;
        if texd_mips == 1 {
            return 0;
        }

        if self.type_ == TextureType::Billboard || self.interpret_as == InterpretAs::UNKNOWN64{
            return 0;
        }

        if self.type_ == TextureType::UNKNOWN512 {
            return 0;
        }

        if self.format == RenderFormat::DXT1 && (self.width as usize * self.height as usize) == 16 {
            return 1;
        }

        let area = self.width as usize * self.height as usize;
        let ret = ((area as f32).log2() * 0.5 - 6.5).floor() as usize;
        ret
    }

    fn size() -> usize {
        152
    }

    fn text_data_size(&self) -> usize {
        let text_mip_levels = self.num_mip_levels as usize - self.get_text_scale();
        let blocks_to_skip = self.num_mip_levels as usize - text_mip_levels;
        let last_mip_size = self.compressed_mip_sizes[(self.num_mip_levels - 1) as usize] as usize;
        if blocks_to_skip == 0 {
            return last_mip_size;
        }
        let texd_mip_size = self.compressed_mip_sizes.get(blocks_to_skip - 1).unwrap_or(&0);
        last_mip_size - *texd_mip_size as usize
    }

    fn has_atlas(&self) -> bool {
        self.has_atlas
    }

    fn texd_mip_levels(&self) -> usize {
        self.num_mip_levels as usize
    }
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[br(import(woa_version: WoaVersion))]
pub enum TextureMap {
    #[br(pre_assert(woa_version == WoaVersion::HM2016))]
    V1(TextureMapInner<TextureMapHeaderV1>),

    #[br(pre_assert(woa_version == WoaVersion::HM2))]
    V2(TextureMapInner<TextureMapHeaderV2>),

    #[br(pre_assert(woa_version == WoaVersion::HM3))]
    V3(TextureMapInner<TextureMapHeaderV3>),
}

impl From<TextureMapInner<TextureMapHeaderV1>> for TextureMap {
    fn from(inner: TextureMapInner<TextureMapHeaderV1>) -> Self {
        TextureMap::V1(inner)
    }
}

impl From<TextureMapInner<TextureMapHeaderV2>> for TextureMap {
    fn from(inner: TextureMapInner<TextureMapHeaderV2>) -> Self {
        TextureMap::V2(inner)
    }
}

impl From<TextureMapInner<TextureMapHeaderV3>> for TextureMap {
    fn from(inner: TextureMapInner<TextureMapHeaderV3>) -> Self {
        TextureMap::V3(inner)
    }
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TextureData {
    Tex(Vec<u8>),
    Mipblock1(MipblockData),
}


impl BinWrite for TextureData {
    type Args<'a> = (usize,);

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        match self {
            TextureData::Tex(data) => {
                writer.write_type(data, endian)
            }
            TextureData::Mipblock1(mipblock) => {
                let data = &mipblock.data;
                let cut_data = &data.clone().into_iter().skip(data.len() - args.0).collect::<Vec<_>>();
                writer.write_type(cut_data, endian)
            }
        }
    }
}

impl TextureData {
    fn size(&self) -> usize {
        match self {
            TextureData::Tex(d) => { d.len() }
            TextureData::Mipblock1(d) => { d.data.len() }
        }
    }
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TilePolygonVertex {
    pos_lerp_x: f32,
    pos_lerp_y: f32,
    text_uv_x: f32,
    text_uv_y: f32,
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AtlasData {
    #[br(temp)]
    #[bw(calc(polygon_vertices.len() as u32 / (width * height)))]
    pub polygon_vertex_count: u32,
    pub width: u32,
    pub height: u32,

    #[br(count = (width * height * polygon_vertex_count) as usize)]
    pub polygon_vertices: Vec<TilePolygonVertex>,
}

impl AtlasData {
    pub(crate) fn size(&self) -> usize {
        (3 * size_of::<u32>()) + (self.polygon_vertices.len() * size_of::<TilePolygonVertex>())
    }
}

#[binread]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextureMapInner<A>
where
    A: for<'a> BinRead<Args<'a>=()>,
    A: TextureMapHeaderImpl,
{
    pub header: A,

    //I would like to seek_before = SeekFrom::Start(TextureMapHeaderArgs::from(header.clone()).atlas_data_offset as u64) here, but H1 and 2 have a -8 offset on the pointer
    #[br(if (header.has_atlas()))]
    pub atlas_data: Option<AtlasData>,

    #[br(parse_with = until_eof, map = TextureData::Tex)]
    #[serde(skip_serializing)]
    pub data: TextureData,
}

impl<A> BinWrite for TextureMapInner<A>
where
    A: for<'a> BinWrite<Args<'a>=((DynamicTextureMapArgs,))> + Clone + for<'a> binrw::BinRead<Args<'a>=()>,
    A: TextureMapHeaderImpl,
{
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, _: Self::Args<'_>) -> BinResult<()> {
        let atlas_size = self.atlas_data_size();
        let total_size = self.data.size()
            + A::size()
            + atlas_size;

        let args = DynamicTextureMapArgs {
            data_size: total_size as u32,
            atlas_data_size: atlas_size as u32,
            text_scale: self.header.get_text_scale() as u8,
            text_mip_levels: self.header.texd_mip_levels() as u8 - self.header.get_text_scale() as u8,
        };
        self.header.write_options(writer, endian, (args,))?;

        // If atlas_data is present, write it
        if let Some(atlas_data) = &self.atlas_data {
            atlas_data.write_options(writer, endian, ())?;
        }

        // Now write the data
        let text_data_size = self.header.text_data_size();
        self.data.write_options(writer, endian, (text_data_size,))?;

        Ok(())
    }
}

impl<A> TextureMapInner<A>
where
    A: for<'a> BinRead<Args<'a>=()>,
    A: Clone,
    A: for<'a> binrw::BinWrite<Args<'a>=((DynamicTextureMapArgs,))>,
    A: TextureMapHeaderImpl,
{
    pub fn get_data(&self) -> &Vec<u8> {
        match &self.data {
            TextureData::Tex(d) => { d }
            TextureData::Mipblock1(d) => { &d.data }
        }
    }

    pub fn get_atlas_data(&self) -> &Option<AtlasData> {
        &self.atlas_data
    }

    fn atlas_data_size(&self) -> usize {
        self.atlas_data.as_ref().map(|atlas| atlas.size()).unwrap_or(0)
    }

    pub fn has_mipblock1_data(&self) -> bool {
        match &self.data {
            TextureData::Tex(_) => { false }
            TextureData::Mipblock1(_) => { true }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MipLevel {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl TextureMap {
    pub fn get_version(&self) -> WoaVersion {
        match self {
            TextureMap::V1(_) => { WoaVersion::HM2016 }
            TextureMap::V2(_) => { WoaVersion::HM2 }
            TextureMap::V3(_) => { WoaVersion::HM3 }
        }
    }

    pub fn get_data(&self) -> &Vec<u8> {
        match self {
            TextureMap::V1(t) => { t.get_data() }
            TextureMap::V2(t) => { t.get_data() }
            TextureMap::V3(t) => { t.get_data() }
        }
    }

    pub(crate) fn get_atlas_data(&self) -> &Option<AtlasData> {
        match self {
            TextureMap::V1(t) => { t.get_atlas_data() }
            TextureMap::V2(t) => { t.get_atlas_data() }
            TextureMap::V3(t) => { t.get_atlas_data() }
        }
    }

    fn set_data(&mut self, data: TextureData) {
        match self {
            TextureMap::V1(t) => { t.data = data }
            TextureMap::V2(t) => { t.data = data }
            TextureMap::V3(t) => { t.data = data }
        }
    }

    fn text_mip_levels(&self) -> usize {
        match self {
            TextureMap::V1(inner) => { inner.header.num_mip_levels as usize - self.text_scale() }
            TextureMap::V2(inner) => { inner.header.num_mip_levels as usize - self.text_scale() }
            TextureMap::V3(inner) => { inner.header.num_mip_levels as usize - self.text_scale() }
        }
    }
    fn texd_mip_levels(&self) -> usize {
        match self {
            TextureMap::V1(inner) => { inner.header.num_mip_levels as usize }
            TextureMap::V2(inner) => { inner.header.num_mip_levels as usize }
            TextureMap::V3(inner) => { inner.header.num_mip_levels as usize }
        }
    }

    pub fn get_num_mip_levels(&self) -> usize {
        let texd_levels = self.texd_mip_levels();
        if self.has_mipblock1_data() {
            texd_levels
        } else {
            match self {
                TextureMap::V1(inner) => { texd_levels - inner.header.get_text_scale() }
                TextureMap::V2(inner) => { texd_levels - inner.header.get_text_scale() }
                TextureMap::V3(inner) => { texd_levels - inner.header.get_text_scale() }
            }
        }
    }

    fn text_scale(&self) -> usize {
        match self {
            TextureMap::V1(tex) => { tex.header.get_text_scale() }
            TextureMap::V2(tex) => { tex.header.get_text_scale() }
            TextureMap::V3(tex) => { tex.header.get_text_scale() }
        }
    }

    fn texd_size(&self) -> (usize, usize) {
        match self {
            TextureMap::V1(tex) => { (tex.header.width as usize, tex.header.height as usize) }
            TextureMap::V2(tex) => { (tex.header.width as usize, tex.header.height as usize) }
            TextureMap::V3(tex) => { (tex.header.width as usize, tex.header.height as usize) }
        }
    }

    fn mip_sizes(&self) -> Vec<u32> {
        match self {
            TextureMap::V1(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMap::V2(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMap::V3(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
        }
    }

    fn compressed_mip_sizes(&self) -> Vec<u32> {
        match self {
            TextureMap::V1(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMap::V2(tex) => { tex.header.compressed_mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMap::V3(tex) => { tex.header.compressed_mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
        }
    }

    pub fn max_video_memory_size(&self) -> u32 {
        self.text_mip_levels();
        self.mip_sizes().first().cloned().unwrap_or(0)
    }

    fn text_size(&self) -> (usize, usize) {
        let (width, height) = self.texd_size();
        let scale_factor = 1 << self.text_scale();
        (width / scale_factor, height / scale_factor)
    }

    pub fn width(&self) -> usize {
        if self.has_mipblock1_data() { self.texd_size().0 } else { self.text_size().0 }
    }

    pub fn height(&self) -> usize {
        if self.has_mipblock1_data() { self.texd_size().1 } else { self.text_size().1 }
    }

    pub fn format(&self) -> RenderFormat {
        match self {
            TextureMap::V1(tex) => { tex.header.format }
            TextureMap::V2(tex) => { tex.header.format }
            TextureMap::V3(tex) => { tex.header.format }
        }
    }

    pub fn dimensions(&self) -> Dimensions {
        match self {
            TextureMap::V1(tex) => { tex.header.dimensions }
            TextureMap::V2(_) => { Dimensions::_2D }
            TextureMap::V3(tex) => { tex.header.dimensions }
        }
    }

    fn has_mipblock1_data(&self) -> bool {
        match self {
            TextureMap::V1(t) => { t.has_mipblock1_data() }
            TextureMap::V2(t) => { t.has_mipblock1_data() }
            TextureMap::V3(t) => { t.has_mipblock1_data() }
        }
    }

    pub fn get_mip_level(&self, level: usize) -> Result<MipLevel, TextureMapError> {
        let removed_mip_count = self.texd_mip_levels() - self.text_mip_levels();

        let mut mips_sizes: Vec<u32> = self.mip_sizes();
        let mut block_sizes: Vec<u32> = self.compressed_mip_sizes();

        if !self.has_mipblock1_data() {
            let removed_mip = mips_sizes.drain(0..removed_mip_count as usize).collect::<Vec<u32>>().pop().unwrap_or(0);
            mips_sizes.iter_mut().for_each(|x| if *x > 0 { *x -= removed_mip });

            let removed_block = block_sizes.drain(0..removed_mip_count as usize).collect::<Vec<u32>>().pop().unwrap_or(0);
            block_sizes.iter_mut().for_each(|x| if *x > 0 { *x -= removed_block });
        }


        if level > self.texd_mip_levels() {
            return Err(TextureMapError::UnknownError("mip level is out of bounds".parse().unwrap()));
        }

        let mip_start = if level > 0 { mips_sizes[level - 1] } else { 0 };
        let mip_size = mips_sizes.get(level).ok_or(TextureMapError::UnknownError("mip level is out of bounds".parse().unwrap()))? - mip_start;

        let block_start = if level > 0 { block_sizes[level - 1] } else { 0 };
        let block_size = block_sizes.get(level).ok_or(TextureMapError::UnknownError("mip level is out of bounds".parse().unwrap()))? - block_start;

        let is_compressed = mip_size != block_size;
        let block = self.get_data().clone().into_iter().skip(block_start as usize).take(block_size as usize).collect::<Vec<u8>>();
        let data = if is_compressed {
            let mut dst = vec![0u8; mip_size as usize];
            match lz4::block::decompress_to_buffer(block.as_slice(), Some(mip_size as i32), &mut dst) {
                Ok(_) => {}
                Err(e) => {
                    println!("brokey {}", e);
                }
            };
            dst
        } else {
            block
        };

        Ok(MipLevel {
            width: self.width() >> level,
            height: self.height() >> level,
            data,
        })
    }

    pub fn set_mipblock1_data(&mut self, texd_data: &Vec<u8>, version: WoaVersion) -> Result<(), TextureMapError> {
        let mipblock = MipblockData::new(texd_data, version)?;
        self.set_data(TextureData::Mipblock1(mipblock));
        Ok(())
    }

    pub fn set_mipblock1(&mut self, mipblock: MipblockData){
        self.set_data(TextureData::Mipblock1(mipblock))
    }

    pub fn has_atlas(&self) -> bool {
        self.has_atlas()
    }

    pub(crate) fn texd_header(&self) -> Result<Vec<u8>, TextureMapError>{
        let mut writer = Cursor::new(Vec::new());

        let data = match self{
            TextureMap::V1(d) => {&d.data}
            TextureMap::V2(d) => {&d.data}
            TextureMap::V3(d) => {&d.data}
        };

        let atlas_size = self.get_atlas_data().as_ref().map(|atlas| atlas.size()).unwrap_or(0);
        let total_size = data.size()
            + match self {
            TextureMap::V1(_) => {TextureMapHeaderV1::size()}
            TextureMap::V2(_) => {TextureMapHeaderV2::size()}
            TextureMap::V3(_) => {TextureMapHeaderV3::size()}
            }
            + atlas_size;

        let args = DynamicTextureMapArgs {
            data_size: total_size as u32,
            atlas_data_size: atlas_size as u32,

            //not needed as these are only used in H3, which doesn't use a texd header.
            text_scale: 0,
            text_mip_levels: 0,
        };
        match self{
            TextureMap::V1(tex) => {tex.header.write_options(&mut writer, Endian::Little, (args,))?}
            TextureMap::V2(tex) => {tex.header.write_options(&mut writer, Endian::Little, (args,))?}
            TextureMap::V3(tex) => {tex.header.write_options(&mut writer, Endian::Little, (args,))?}
        }

        // If atlas_data is present, write it
        if let Some(atlas_data) = &self.get_atlas_data() {
            atlas_data.write_options(&mut writer, Endian::Little, ())?;
        }

        Ok(writer.into_inner())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MipblockData {
    pub header: Vec<u8>,
    pub data: Vec<u8>,
}

impl From<MipblockData> for Vec<u8> {
    fn from(value: MipblockData) -> Self {
        value.data
    }
}

impl MipblockData{
    pub fn new(data: &Vec<u8>, version: WoaVersion) -> Result<Self, TextureMapError>{
        let mut stream = Cursor::new(data);

        let read_size = match version {
            WoaVersion::HM2016 => {
                stream.set_position(8);
                let data_size = stream.read_le::<u32>()?;
                stream.set_position(0);

                let texd_header = TextureMapHeaderV1::read_le_args(&mut stream, ())?;
                let mut atlas: Option<AtlasData> = None;
                if texd_header.has_atlas {
                    atlas = Some(AtlasData::read_le(&mut stream)?);
                }
                data_size as usize - (TextureMapHeaderV1::size() - 8) - atlas.map(|a| a.size()).unwrap_or(0)
            }
            WoaVersion::HM2 => {
                stream.set_position(4);
                let data_size = stream.read_le::<u32>()?;
                stream.set_position(0);

                let texd_header = TextureMapHeaderV2::read_le_args(&mut stream, ())?;
                let mut atlas: Option<AtlasData> = None;
                if texd_header.has_atlas {
                    atlas = Some(AtlasData::read_le(&mut stream)?);
                }
                data_size as usize - (TextureMapHeaderV2::size()) - atlas.map(|a| a.size()).unwrap_or(0)
            }
            WoaVersion::HM3 => {
                data.len()
            }
        };

        let mut buffer = vec![0u8; read_size];
        stream.read_exact(&mut buffer)?;
        Ok(Self{
            header: vec![],
            data: buffer,
        })
    }

    pub(crate) fn insert_text_header(&mut self, texture_map: &TextureMap)
    {
        if let Ok(header) = texture_map.texd_header(){
            self.header = header;
        }
    }
}