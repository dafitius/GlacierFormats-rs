use std::fs::File;
use std::{fs, io};
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use binrw::{BinRead, binread, BinResult, binrw, BinWrite, BinWriterExt, Endian};
use binrw::helpers::until_eof;
use bitfield_struct::bitfield;
use directxtex::{DXGI_FORMAT, TEX_DIMENSION};
use crate::WoaVersion;
use crate::texture_map::TextureData::Mipblock1;


#[derive(Debug, thiserror::Error)]
pub enum TextureMapError {
    #[error("Io error")]
    IoError(#[from] io::Error),

    #[error("Parsing error")]
    ParsingError(#[from] binrw::Error),

    #[error("Failed on {0}")]
    UnknownError(String),
}

#[derive(BinRead, BinWrite, Debug, Copy, Clone, PartialEq)]
#[brw(repr = u8)]
pub enum TextureType
{
    Colour = 0,
    Normal = 1,
    Height = 2,
    CompoundNormal = 3,
    Billboard = 4,
    Projection = 6,
    Emission = 16,
    UNKNOWN64 = 64,
}

#[derive(BinRead, BinWrite, Debug, Copy)]
#[brw(repr = u16)]
#[derive(Clone, PartialEq)]
pub enum RenderFormat
{
    R16G16B16A16 = 0x0A,

    R8G8B8A8 = 0x1C,
    //Normals. very rarely used. Legacy? Only one such tex in chunk0;
    R8G8 = 0x34,
    //8-bit grayscale uncompressed. Not used on models?? Overlays
    A8 = 0x42,
    //Color maps, 1-bit alpha (mask). Many uses, color, normal, spec, rough maps on models and decals. Also used as masks.
    DXT1 = 0x49,

    DXT3 = 0x4C,
    //Packed color, full alpha. Similar use as DXT5.
    DXT5 = 0x4F,
    //8-bit grayscale. Few or no direct uses on models?
    BC4 = 0x52,
    //2-channel normal maps
    BC5 = 0x55,
    //hi-res color + full alpha. Used for pretty much everything...
    BC7 = 0x5A,
}

impl RenderFormat {
    pub fn is_compressed(&self) -> bool {
        matches!(self, RenderFormat::DXT1|
            RenderFormat::DXT3|
            RenderFormat::DXT5|
            RenderFormat::BC4|
            RenderFormat::BC5|
            RenderFormat::BC7)
    }

    pub fn num_channels(&self) -> usize {
        match self {
            RenderFormat::A8 | RenderFormat::BC4 => 1,
            RenderFormat::R8G8 | RenderFormat::BC5 => 2,
            RenderFormat::DXT1 | //assume DXT1a
            RenderFormat::R16G16B16A16 |
            RenderFormat::R8G8B8A8 |
            RenderFormat::DXT3 |
            RenderFormat::DXT5 |
            RenderFormat::BC7 => 4,
        }
    }
}

impl From<RenderFormat> for DXGI_FORMAT {
    fn from(value: RenderFormat) -> Self {
        match value {
            RenderFormat::R16G16B16A16 => { DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_UNORM }
            RenderFormat::R8G8B8A8 => { DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM }
            RenderFormat::R8G8 => { DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM }
            RenderFormat::A8 => { DXGI_FORMAT::DXGI_FORMAT_A8_UNORM }
            RenderFormat::DXT1 => { DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM }
            RenderFormat::DXT3 => { DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM }
            RenderFormat::DXT5 => { DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM }
            RenderFormat::BC4 => { DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM }
            RenderFormat::BC5 => { DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM }
            RenderFormat::BC7 => { DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM }
        }
    }
}

#[derive(BinRead, BinWrite, Debug, Copy, Clone, PartialEq)]
#[brw(repr = u8)]
pub enum Dimensions
{
    _2D = 0,
    Cube = 1,
    Volume = 2,
}

impl From<Dimensions> for TEX_DIMENSION {
    fn from(val: Dimensions) -> Self {
        match val {
            Dimensions::_2D => { TEX_DIMENSION::TEX_DIMENSION_TEXTURE2D }
            Dimensions::Cube => { TEX_DIMENSION::TEX_DIMENSION_TEXTURE3D }
            Dimensions::Volume => { TEX_DIMENSION::TEX_DIMENSION_TEXTURE2D }
        }
    }
}

#[bitfield(u32)]
#[derive(BinRead, BinWrite)]
//#[brw(repr = u32)]
//most of these are unused...
pub struct RenderResourceMiscFlags
{
    persistent_data: bool,
    pub(crate) texture_cube: bool,
    texture_normalmap: bool,
    pub(crate) texture_swizzled: bool,
    pub(crate) temp_alloc: bool,
    unused2: bool,
    pub(crate) no_color_compression: bool,
    has_analysis_data: bool,
    texture_srgb: bool,
    force_main_mem: bool,
    shared: bool,
    buffer_structured: bool,
    linear_data: bool,
    esram_resolve_present: bool,
    draw_indirect_args: bool,
    no_stencil: bool,
    texture_streamer: bool,
    dx11afr_dont_copy: bool,
    dx12afr_mgpu_visible: bool,
    ms: bool,
    ps4_no_h_tile: bool,

    #[bits(11)]
    __: u32,
}

pub struct TextureMapHeader {
    pub type_: TextureType,
    pub flags: RenderResourceMiscFlags,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub format: RenderFormat,
    pub(crate) default_mip_level: u8,
    pub interpret_as: TextureType,
    pub dimensions: Dimensions,
}

#[derive(Debug)]
pub struct TextureMapHeaderArgs {
    pub(crate) mips_sizes: Vec<u32>,
    pub(crate) block_sizes: Vec<u32>,
    atlas_data_size: u32,
    atlas_data_offset: u32,
    pub(crate) texd_width: usize,
    pub(crate) texd_height: usize,
    pub(crate) num_mips_levels: u8,
    pub(crate) text_mips_levels: u8,
}

pub struct MipLevel {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

#[binrw]
#[derive(Debug)]
#[derive(Clone)]
#[br(assert(num_textures == 1 && num_textures != 6, "Looks like you tried to export a cubemap texture, those are not supported yet"))]
pub struct TextureMapHeaderV1 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    #[brw(pad_after = 1)]
    type_: TextureType,

    #[br(temp)]
    #[bw(calc(0xFF))]
    texd_identifier: u32,
    #[br(temp)]
    #[bw(calc(69))]
    file_size: u32,
    flags: RenderResourceMiscFlags,
    width: u16,
    height: u16,
    format: RenderFormat,
    num_mips_levels: u8,
    default_mip_level: u8,
    interpret_as: TextureType,
    dimensions: Dimensions,
    #[br(temp)]
    #[bw(calc(0))]
    mips_interpolation_deprecated: u16,
    mips_sizes: [u32; 0xE],
    atlas_data_size: u32,
    atlas_data_offset: u32,
}

impl TextureMapHeaderV1 {
    pub(crate) fn get_text_scale(&self) -> usize
    {
        let texd_mips = self.num_mips_levels as usize;

        if texd_mips == 1 {
            return 0;
        }

        if self.interpret_as == TextureType::Billboard {
            return 0;
        }

        if self.interpret_as == TextureType::UNKNOWN64 {
            //TODO: Delete this when it's confirmed that it doesn't exist
            println!("DELETE ME!!!!!");
            return 0;
        }

        let area = self.width as usize * self.height as usize;
        ((area as f32).log2() * 0.5 - 6.5).floor() as usize
    }
}

impl From<TextureMapHeaderV1> for TextureMapHeader {
    fn from(val: TextureMapHeaderV1) -> Self {
        let text_scale = val.get_text_scale();
        TextureMapHeader {
            type_: val.type_,
            flags: val.flags,
            width: val.width as usize / 2usize.pow(text_scale as u32),
            height: val.height as usize / 2usize.pow(text_scale as u32),
            format: val.format,
            default_mip_level: val.default_mip_level,
            interpret_as: val.interpret_as,
            dimensions: val.dimensions,
        }
    }
}

impl From<TextureMapHeaderV1> for TextureMapHeaderArgs {
    fn from(value: TextureMapHeaderV1) -> Self {
        TextureMapHeaderArgs {
            mips_sizes: value.mips_sizes.into_iter().filter(|x| *x != 0).collect(),
            block_sizes: value.mips_sizes.into_iter().filter(|x| *x != 0).collect(),
            atlas_data_size: value.atlas_data_size,
            atlas_data_offset: value.atlas_data_offset,
            texd_width: value.width as usize,
            texd_height: value.height as usize,
            num_mips_levels: value.num_mips_levels,
            text_mips_levels: value.num_mips_levels - value.get_text_scale() as u8,
        }
    }
}

#[binrw]
#[derive(Debug)]
#[derive(Clone)]
#[br(assert(mips_sizes == mips_sizes_dup))]
#[br(assert(num_textures == 1))]
pub struct TextureMapHeaderV2 {
    num_textures: u16,
    #[brw(pad_after = 1)]
    type_: TextureType,
    //#[br(temp)]
    //#[bw(calc(69))]
    file_size: u32,
    flags: RenderResourceMiscFlags,
    width: u16,
    height: u16,
    format: RenderFormat,
    num_mips_levels: u8,
    default_mip_level: u8,
    texd_identifier: u32,
    mips_sizes: [u32; 0xE],
    mips_sizes_dup: [u32; 0xE],
    atlas_data_size: u32,
    atlas_data_offset: u32,
}

impl TextureMapHeaderV2 {
    pub(crate) fn get_text_scale(&self) -> usize
    {
        let texd_mips = self.num_mips_levels as usize;
        if texd_mips == 1 {
            return 0;
        }

        if self.type_ == TextureType::Billboard {
            return 0;
        }

        if self.type_ == TextureType::UNKNOWN64 {
            //TODO: Delete this when it's confirmed that it doesn't exist
            println!("DELETE ME!!!!!");
            return 0;
        }

        if self.format == RenderFormat::DXT1 && self.width * self.height == 16 {
            return 1;
        }

        if self.texd_identifier != 16384 {
            return 0;
        }

        let area = self.width as usize * self.height as usize;
        ((area as f32).log2() * 0.5 - 6.5).floor() as usize
    }
}

impl From<TextureMapHeaderV2> for TextureMapHeader {
    fn from(val: TextureMapHeaderV2) -> Self {
        let text_scale = val.get_text_scale();
        TextureMapHeader {
            type_: val.type_,
            flags: val.flags,
            width: val.width as usize / 2usize.pow(text_scale as u32),
            height: val.height as usize / 2usize.pow(text_scale as u32),
            format: val.format,
            default_mip_level: val.default_mip_level,
            interpret_as: val.type_,
            dimensions: Dimensions::_2D,
        }
    }
}

impl From<TextureMapHeaderV2> for TextureMapHeaderArgs {
    fn from(value: TextureMapHeaderV2) -> Self {
        let text_scale = value.get_text_scale();
        TextureMapHeaderArgs {
            mips_sizes: value.mips_sizes.into_iter().filter(|x| *x != 0).collect(),
            block_sizes: value.mips_sizes_dup.into_iter().filter(|x| *x != 0).collect(),
            atlas_data_size: value.atlas_data_size,
            atlas_data_offset: value.atlas_data_offset,
            texd_width: value.width as usize,
            texd_height: value.height as usize,
            num_mips_levels: value.num_mips_levels,
            text_mips_levels: value.num_mips_levels - text_scale as u8,
        }
    }
}

#[binrw]
#[derive(Debug)]
#[derive(Clone)]
#[br(assert(text_scaling_width == num_mips_levels - text_mips_levels))]
#[br(assert(text_scaling_height == num_mips_levels - text_mips_levels))]
#[br(assert(num_textures == 1))]
#[bw(import())]
pub struct TextureMapHeaderV3 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,
    #[brw(pad_after = 1)]
    type_: TextureType,

    //#[br(temp)]
    //#[bw(calc(self.get_data_size()))]
    data_size: u32,
    flags: RenderResourceMiscFlags,
    width: u16,
    height: u16,
    format: RenderFormat,
    num_mips_levels: u8,
    default_mip_level: u8,
    interpret_as: TextureType,
    dimensions: Dimensions,

    #[br(temp)]
    #[bw(calc(0))]
    mips_interpolation_deprecated: u16,
    mips_sizes: [u32; 0xE],
    block_sizes: [u32; 0xE],
    atlas_data_size: u32,
    atlas_data_offset: u32,
    #[br(temp)]
    #[bw(calc(0xFF))]
    text_scaling_data1: u8,
    text_scaling_width: u8,
    text_scaling_height: u8,
    #[brw(pad_after = 0x4)]
    text_mips_levels: u8,
}

impl From<TextureMapHeaderV3> for TextureMapHeader {
    fn from(val: TextureMapHeaderV3) -> Self {
        TextureMapHeader {
            type_: val.type_,
            flags: val.flags,
            width: val.width as usize / 2usize.pow(val.text_scaling_width as u32),
            height: val.height as usize / 2usize.pow(val.text_scaling_height as u32),
            format: val.format,
            default_mip_level: val.default_mip_level,
            interpret_as: val.interpret_as,
            dimensions: val.dimensions,
        }
    }
}

impl From<TextureMapHeaderV3> for TextureMapHeaderArgs {
    fn from(value: TextureMapHeaderV3) -> Self {
        TextureMapHeaderArgs {
            mips_sizes: value.mips_sizes.into_iter().filter(|x| *x != 0).collect(),
            block_sizes: value.block_sizes.into_iter().filter(|x| *x != 0).collect(),
            atlas_data_size: value.atlas_data_size,
            atlas_data_offset: value.atlas_data_offset,
            texd_width: value.width as usize,
            texd_height: value.height as usize,
            num_mips_levels: value.num_mips_levels,
            text_mips_levels: value.text_mips_levels,
        }
    }
}


#[binread]
#[derive(Debug)]
#[br(import(woa_version: WoaVersion))]
pub enum TextureMap {
    #[br(pre_assert(woa_version == WoaVersion::HM2016))]
    V1(TextureMapInner<TextureMapHeaderV1>),

    #[br(pre_assert(woa_version == WoaVersion::HM2))]
    V2(TextureMapInner<TextureMapHeaderV2>),

    #[br(pre_assert(woa_version == WoaVersion::HM3))]
    V3(TextureMapInner<TextureMapHeaderV3>),
}

impl BinWrite for TextureMap {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        match self {
            TextureMap::V1(h) => { writer.write_type(h, Endian::Little)? }
            TextureMap::V2(h) => { writer.write_type(h, Endian::Little)? }
            TextureMap::V3(h) => { writer.write_type(h, Endian::Little)? }
        }

        let file_size = writer.stream_position()?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum TextureData {
    Tex(Vec<u8>),
    Mipblock1(Vec<u8>),
}

impl BinWrite for TextureData {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, args: Self::Args<'_>) -> BinResult<()> {
        match self {
            TextureData::Tex(data) => { writer.write_type(data, endian) }
            Mipblock1(_) => { todo!() }
        }
    }
}

#[binrw]
#[derive(Debug)]
pub struct AtlasEntry {
    left_up_unk1: f32,
    left_up_unk2: f32,
    left_up_x: f32,
    left_up_y: f32,

    right_up_unk1: f32,
    right_up_unk2: f32,
    right_up_x: f32,
    right_up_y: f32,

    right_down_unk1: f32,
    right_down_unk2: f32,
    right_down_x: f32,
    right_down_y: f32,

    left_down_unk1: f32,
    left_down_unk2: f32,
    left_down_x: f32,
    left_down_y: f32,
}

#[binrw]
#[derive(Debug)]
#[br(import(total_size: u32))]
#[br(assert(total_size - 12 == (width * height) * 0x40, "oops! u are wrong"))]
#[br(assert(unk1 == 4, "Oh wow, it's not equal to four, it's {}", unk1))]
pub struct AtlasData {
    pub unk1: u32,
    pub width: u32,
    pub height: u32,

    #[br(count = (width * height) as usize)]
    pub entries: Vec<AtlasEntry>,
}

#[binrw]
#[derive(Debug)]
pub struct TextureMapInner<
    A: for<'a> BinRead<Args<'a>=()> + for<'a> BinWrite<Args<'a>=()> + Clone
> where TextureMapHeaderArgs: From<A> {
    pub header: A,

    //I would like to seek_before = SeekFrom::Start(TextureMapHeaderArgs::from(header.clone()).atlas_data_offset as u64) here, but H1 and 2 have a -8 offset on the pointer
    #[brw(if (TextureMapHeaderArgs::from(header.clone()).atlas_data_size > 0))]
    #[br(args(TextureMapHeaderArgs::from(header.clone()).atlas_data_size))]
    pub atlas_data: Option<AtlasData>,

    #[br(parse_with = until_eof, map = TextureData::Tex)]
    pub data: TextureData,
}

impl<A: for<'a> BinRead<Args<'a>=()> + Clone + for<'a> binrw::BinWrite<Args<'a>=()>> TextureMapInner<A> where TextureMapHeaderArgs: From<A> {
    pub fn get_data(&self) -> &Vec<u8> {
        match &self.data {
            TextureData::Tex(d) => { d }
            Mipblock1(d) => { d }
        }
    }

    pub fn get_atlas_data(&self) -> &Option<AtlasData> {
        &self.atlas_data
    }

    pub fn has_mipblock1_data(&self) -> bool {
        match &self.data {
            TextureData::Tex(_) => { false }
            Mipblock1(_) => { true }
        }
    }
}


impl TextureMap {
    pub fn get_header(&self) -> TextureMapHeader {
        match self {
            TextureMap::V1(t) => { t.header.clone().into() }
            TextureMap::V2(t) => { t.header.clone().into() }
            TextureMap::V3(t) => { t.header.clone().into() }
        }
    }

    pub fn get_version(&self) -> WoaVersion {
        match self {
            TextureMap::V1(_) => { WoaVersion::HM2016 }
            TextureMap::V2(_) => { WoaVersion::HM2 }
            TextureMap::V3(_) => { WoaVersion::HM3 }
        }
    }

    //TODO: Private this
    pub fn get_header_args(&self) -> TextureMapHeaderArgs {
        match self {
            TextureMap::V1(t) => { t.header.clone().into() }
            TextureMap::V2(t) => { t.header.clone().into() }
            TextureMap::V3(t) => { t.header.clone().into() }
        }
    }

    //TODO: Private this
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

    pub fn get_num_mip_levels(&self) -> usize {
        if self.has_mipblock1_data() {
            self.get_header_args().num_mips_levels as usize
        } else {
            self.get_header_args().text_mips_levels as usize
        }
    }

    pub fn width(&self) -> usize {
        if self.has_mipblock1_data() { self.get_header_args().texd_width } else { self.get_header().width }
    }

    pub fn height(&self) -> usize {
        if self.has_mipblock1_data() { self.get_header_args().texd_height } else { self.get_header().height }
    }

    fn has_mipblock1_data(&self) -> bool {
        match self {
            TextureMap::V1(t) => { t.has_mipblock1_data() }
            TextureMap::V2(t) => { t.has_mipblock1_data() }
            TextureMap::V3(t) => { t.has_mipblock1_data() }
        }
    }

    pub(crate) fn get_text_scaling(&self) -> usize
    {
        let texd_mips = self.get_header_args().num_mips_levels as usize;
        if texd_mips == 1 {
            return texd_mips;
        }

        if if self.get_version() == WoaVersion::HM3 { self.get_header().type_ } else { self.get_header().interpret_as } == TextureType::Billboard {
            return texd_mips;
        }

        if self.get_header().interpret_as == TextureType::UNKNOWN64 {
            return texd_mips;
        }

        if self.get_version() == WoaVersion::HM2 && self.get_header().format == RenderFormat::DXT1 && self.get_header_args().texd_width * self.get_header_args().texd_height == 16 {
            return texd_mips - 1;
        }

        if let TextureMap::V2(g) = self{
            if g.header.texd_identifier != 16384 {
                return texd_mips;
            }
        }

        let area = self.get_header_args().texd_width * self.get_header_args().texd_height;
        ((area as f32).log2() * 0.5 - 6.5).floor() as usize
    }

    pub(crate) fn derive_text_mip_level(&self) -> Option<usize> {
        let mips_sizes: Vec<u32> = self.get_header_args().block_sizes.to_vec();

        let mips = mips_sizes.iter().enumerate().map(|(i, x)| if x > &0 {
            if i == 0 {
                *mips_sizes.first().unwrap()
            } else {
                x - mips_sizes.get(i - 1).unwrap_or(&0)
            }
        } else { 0 }).collect::<Vec<_>>();


        let mut sizes = Vec::new();
        let mut sum = 0;
        let mut mip_sizes = mips;
        mip_sizes.reverse();

        for &num in mip_sizes.iter() {
            sum += num;
            sizes.push(sum);
        }

        fn index_of(vec: Vec<u32>, item: u32, padding: usize) -> Option<usize> {
            for (index, value) in vec.iter().enumerate() {
                if item.abs_diff(*value) <= padding as u32 {
                    return Some(index);
                }
            }
            None
        }

        if let Some(text_mip_count) = index_of(sizes.clone(), self.get_data().len() as u32, 8) {
            return Some(text_mip_count + 1);
        } else if self.has_atlas() {
            //println!("couldn't find mip count, because of an atlas");
            return None;
        }
        None
    }

    pub fn get_mip_level(&self, level: usize) -> Result<MipLevel, TextureMapError> {
        let header = self.get_header();
        let args = self.get_header_args();

        let removed_mip_count = args.num_mips_levels - args.text_mips_levels;

        let mut mips_sizes: Vec<u32> = args.mips_sizes.to_vec();
        let mut block_sizes: Vec<u32> = args.block_sizes.to_vec();

        if !self.has_mipblock1_data() {
            let removed_mip = mips_sizes.drain(0..removed_mip_count as usize).collect::<Vec<u32>>().pop().unwrap_or(0);
            mips_sizes.iter_mut().for_each(|x| if *x > 0 { *x -= removed_mip });

            let removed_block = block_sizes.drain(0..removed_mip_count as usize).collect::<Vec<u32>>().pop().unwrap_or(0);
            block_sizes.iter_mut().for_each(|x| if *x > 0 { *x -= removed_block });
        }


        if level > args.num_mips_levels as usize {
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
            width: if self.has_mipblock1_data() { args.texd_width >> level } else { header.width >> level },
            height: if self.has_mipblock1_data() { args.texd_height >> level } else { header.height >> level },
            data,
        })
    }

    pub fn set_mipblock1_data(&mut self, texd_data: &Vec<u8>, version: WoaVersion) -> Result<(), TextureMapError> {
        let mut stream = Cursor::new(texd_data);

        match version {
            WoaVersion::HM2016 => {
                let header = TextureMapHeaderV1::read_le_args(&mut stream, ())?;
                stream.set_position(stream.position() + header.atlas_data_size as u64);
            }
            WoaVersion::HM2 => {
                let header = TextureMapHeaderV2::read_le_args(&mut stream, ())?;
                stream.set_position(stream.position() + header.atlas_data_size as u64);
            }
            _ => {}
        }
        let data: Vec<u8> = until_eof(&mut stream, Endian::Little, ())?;
        self.set_data(Mipblock1(data));
        Ok(())
    }

    pub fn has_atlas(&self) -> bool {
        self.get_header_args().atlas_data_size > 0
    }
}

fn max_mips_count(width: usize, height: usize) -> usize {
    let mip_levels = std::iter::successors(Some((width, height)), |&(w, h)| {
        Some((w.saturating_div(2), h.saturating_div(2))).filter(|&(w, h)| w > 0 || h > 0)
    }).count();

    mip_levels.min(0xE)
}