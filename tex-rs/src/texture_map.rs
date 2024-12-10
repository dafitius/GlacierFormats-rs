#![allow(unused_variables)]

use std::{fs, io};
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Seek, Write};
use std::path::Path;
use binrw::{BinRead, binread, BinResult, binrw, BinWrite, BinWriterExt, Endian};
use binrw::helpers::until_eof;
use serde::{Deserialize, Serialize};
use crate::atlas::AtlasData;
use crate::enums::*;
use crate::mipblock::MipblockData;
use crate::pack::TexturePackerError;
use crate::WoaVersion;

/// Represents the maximum number of mip levels supported.
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

/// Arguments used for dynamically constructing texture map headers.
pub(crate) struct DynamicTextureMapArgs {
    pub(crate) data_size: u32,

    pub(crate) atlas_data_size: u32,

    pub(crate) text_scale: u8,
    pub(crate) text_mip_levels: u8,
}

/// Trait that defines common functionality for texture map headers.
pub(crate) trait TextureMapHeaderImpl {
    /// Calculates the texture scaling factor.
    fn text_scale(&self) -> usize;
    /// Returns the size of the texture map header.
    fn size() -> usize;
    /// Calculates the size of the texture data.
    fn text_data_size(&self) -> usize;
    /// Indicates whether the texture has atlas data.
    fn has_atlas(&self) -> bool;
    /// Returns the number of mip levels in the texture.
    fn texd_mip_levels(&self) -> usize;
}

/// Texture map header for version 1 (HM2016).
#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[br(assert(
    num_textures == 1 && num_textures != 6, "Looks like you tried to export a cubemap texture, those are not supported yet"
))]
#[bw(import(args: DynamicTextureMapArgs))]
pub(crate) struct TextureMapHeaderV1 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    pub(crate) type_: TextureType,

    pub(crate) texd_identifier: u32,

    #[br(temp)]
    #[bw(calc(args.data_size - 8))]
    data_size: u32,
    pub(crate) flags: TextureFlagsInner,
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
    fn text_scale(&self) -> usize
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
        let text_mip_levels = self.num_mip_levels as usize - self.text_scale();
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
pub(crate) struct TextureMapHeaderV2 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    pub(crate) type_: TextureType,

    #[br(temp)]
    #[bw(calc(args.data_size))]
    data_size: u32,
    pub(crate) flags: TextureFlagsInner,
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
    fn text_scale(&self) -> usize
    {
        let texd_mips = self.num_mip_levels as usize;
        if texd_mips == 1 {
            return 0;
        }

        if self.type_ == TextureType::Billboard {
            return 0;
        }

        if self.format == RenderFormat::BC1 && (self.width as usize * self.height as usize) == 16 {
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
        let text_mip_levels = self.num_mip_levels as usize - self.text_scale();
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
pub(crate) struct TextureMapHeaderV3 {
    #[br(temp)]
    #[bw(calc(1))]
    num_textures: u16,

    pub(crate) type_: TextureType,

    #[br(temp)]
    #[bw(calc(args.data_size))]
    data_size: u32,
    pub(crate) flags: TextureFlagsInner,
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
    fn text_scale(&self) -> usize {
        let texd_mips = self.num_mip_levels as usize;
        if texd_mips == 1 {
            return 0;
        }

        if self.type_ == TextureType::Billboard || self.interpret_as == InterpretAs::Volume{
            return 0;
        }

        if self.type_ == TextureType::UNKNOWN512 {
            return 0;
        }

        if self.format == RenderFormat::BC1 && (self.width as usize * self.height as usize) == 16 {
            return 1;
        }

        let area = self.width as usize * self.height as usize;
        ((area as f32).log2() * 0.5 - 6.5).floor() as usize
    }

    fn size() -> usize {
        152
    }

    fn text_data_size(&self) -> usize {
        let text_mip_levels = self.num_mip_levels as usize - self.text_scale();
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
pub struct TextureMap {
    #[br(args(woa_version))]
    pub(crate) inner: TextureMapVersion,
}

#[binrw]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[br(import(woa_version: WoaVersion))]
pub(crate) enum TextureMapVersion {
    #[br(pre_assert(woa_version == WoaVersion::HM2016))]
    V1(TextureMapInner<TextureMapHeaderV1>),

    #[br(pre_assert(woa_version == WoaVersion::HM2))]
    V2(TextureMapInner<TextureMapHeaderV2>),

    #[br(pre_assert(woa_version == WoaVersion::HM3))]
    V3(TextureMapInner<TextureMapHeaderV3>),
}

impl From<TextureMapInner<TextureMapHeaderV1>> for TextureMap {
    fn from(inner: TextureMapInner<TextureMapHeaderV1>) -> Self {
        Self{
            inner: TextureMapVersion::V1(inner)
        }
    }
}

impl From<TextureMapInner<TextureMapHeaderV2>> for TextureMap {
    fn from(inner: TextureMapInner<TextureMapHeaderV2>) -> Self {
        Self{
            inner: TextureMapVersion::V2(inner)
        }
    }
}

impl From<TextureMapInner<TextureMapHeaderV3>> for TextureMap {
    fn from(inner: TextureMapInner<TextureMapHeaderV3>) -> Self {
        Self{
            inner: TextureMapVersion::V3(inner)
        }
    }
}


/// Represents the texture data, which can be either raw texture data or a mipblock read from a texd file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TextureData {
    /// Raw texture data.
    Tex(Vec<u8>),
    /// Mipblock data (obtained from a TEXD resource).
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

#[binread]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct TextureMapInner<A>
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
    A: for<'a> BinWrite<Args<'a>=(DynamicTextureMapArgs,)> + Clone + for<'a> binrw::BinRead<Args<'a>=()>,
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
            text_scale: self.header.text_scale() as u8,
            text_mip_levels: self.header.texd_mip_levels() as u8 - self.header.text_scale() as u8,
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
    A: for<'a> binrw::BinWrite<Args<'a>=(DynamicTextureMapArgs,)>,
    A: TextureMapHeaderImpl,
{
    pub fn data(&self) -> &Vec<u8> {
        match &self.data {
            TextureData::Tex(d) => { d }
            TextureData::Mipblock1(d) => { &d.data }
        }
    }

    pub fn atlas_data(&self) -> &Option<AtlasData> {
        &self.atlas_data
    }

    fn atlas_data_size(&self) -> usize {
        self.atlas_data.as_ref().map(|atlas| atlas.size()).unwrap_or(0)
    }

    pub fn has_mipblock_data(&self) -> bool {
        match &self.data {
            TextureData::Tex(_) => { false }
            TextureData::Mipblock1(_) => { true }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MipLevel {
    pub format: RenderFormat,
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl TextureMap {

    pub fn default_mip_level(&self) -> u8{
        match &self.inner{
            TextureMapVersion::V1(tex) => {tex.header.default_mip_level}
            TextureMapVersion::V2(tex) => {tex.header.default_mip_level}
            TextureMapVersion::V3(tex) => {tex.header.default_mip_level}
        }
    }

    pub fn version(&self) -> WoaVersion {
        match &self.inner{
            TextureMapVersion::V1(_) => { WoaVersion::HM2016 }
            TextureMapVersion::V2(_) => { WoaVersion::HM2 }
            TextureMapVersion::V3(_) => { WoaVersion::HM3 }
        }
    }

    pub(crate) fn data(&self) -> &Vec<u8> {
        match &self.inner {
            TextureMapVersion::V1(t) => { t.data() }
            TextureMapVersion::V2(t) => { t.data() }
            TextureMapVersion::V3(t) => { t.data() }
        }
    }

    pub fn atlas(&self) -> &Option<AtlasData> {
        match &self.inner {
            TextureMapVersion::V1(t) => { t.atlas_data() }
            TextureMapVersion::V2(t) => { t.atlas_data() }
            TextureMapVersion::V3(t) => { t.atlas_data() }
        }
    }

    fn set_data(&mut self, data: TextureData) {
        match &mut self.inner {
            TextureMapVersion::V1(t) => { t.data = data }
            TextureMapVersion::V2(t) => { t.data = data }
            TextureMapVersion::V3(t) => { t.data = data }
        }
    }

    fn text_mip_levels(&self) -> usize {
        self.texd_mip_levels() - self.text_scale()
    }

    fn texd_mip_levels(&self) -> usize {
        match &self.inner {
            TextureMapVersion::V1(inner) => { inner.header.num_mip_levels as usize }
            TextureMapVersion::V2(inner) => { inner.header.num_mip_levels as usize }
            TextureMapVersion::V3(inner) => { inner.header.num_mip_levels as usize }
        }
    }

    pub fn num_mip_levels(&self) -> usize {
        if self.has_mipblock1() {
            self.texd_mip_levels()
        } else {
            self.text_mip_levels()
        }
    }

    fn text_scale(&self) -> usize {
        match &self.inner {
            TextureMapVersion::V1(tex) => { tex.header.text_scale() }
            TextureMapVersion::V2(tex) => { tex.header.text_scale() }
            TextureMapVersion::V3(tex) => { tex.header.text_scale() }
        }
    }

    fn mip_sizes(&self) -> Vec<u32> {
        match &self.inner {
            TextureMapVersion::V1(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMapVersion::V2(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMapVersion::V3(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
        }
    }

    fn compressed_mip_sizes(&self) -> Vec<u32> {
        match &self.inner {
            TextureMapVersion::V1(tex) => { tex.header.mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMapVersion::V2(tex) => { tex.header.compressed_mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
            TextureMapVersion::V3(tex) => { tex.header.compressed_mip_sizes.iter().copied().filter(|mip| *mip != 0).collect() }
        }
    }

    pub fn video_memory_requirement(&self) -> usize {
        match self.version(){
            WoaVersion::HM2016 |
            WoaVersion::HM2 => {
                self.mip_sizes().get(self.text_scale()).cloned().unwrap_or(0) as usize //The size of the largest TEXT mip
            }
            WoaVersion::HM3 => {
                if self.has_mipblock1(){ //if texture has a TEXD
                    (self.mip_sizes().first().cloned().unwrap_or(0) + self.mip_sizes().get(1).cloned().unwrap_or(0)) as usize //the size of the largest two TEXD mip
                } else {0}
            }
        }
    }

    pub fn mipblock1(&self) -> Option<MipblockData> {
        self.has_mipblock1().then(|| {
            self.texd_header().ok().map(|header| MipblockData {
                video_memory_requirement: self.mip_sizes().first().copied().unwrap_or(0x0) as usize,
                header,
                data: self.data().clone(),
            })
        }).flatten()
    }

    fn texd_size(&self) -> (usize, usize) {
        match &self.inner {
            TextureMapVersion::V1(tex) => { (tex.header.width as usize, tex.header.height as usize) }
            TextureMapVersion::V2(tex) => { (tex.header.width as usize, tex.header.height as usize) }
            TextureMapVersion::V3(tex) => { (tex.header.width as usize, tex.header.height as usize) }
        }
    }

    fn text_size(&self) -> (usize, usize) {
        let (width, height) = self.texd_size();
        let scale_factor = 1 << self.text_scale();
        (width / scale_factor, height / scale_factor)
    }

    pub fn width(&self) -> usize {
        if self.has_mipblock1() { self.texd_size().0 } else { self.text_size().0 }
    }

    pub fn height(&self) -> usize {
        if self.has_mipblock1() { self.texd_size().1 } else { self.text_size().1 }
    }

    pub fn format(&self) -> RenderFormat {
        match &self.inner {
            TextureMapVersion::V1(tex) => { tex.header.format }
            TextureMapVersion::V2(tex) => { tex.header.format }
            TextureMapVersion::V3(tex) => { tex.header.format }
        }
    }

    pub fn flags(&self) -> TextureFlags {
        match &self.inner {
            TextureMapVersion::V1(tex) => {TextureFlags{inner: tex.header.flags}}
            TextureMapVersion::V2(tex) => {TextureFlags{inner: tex.header.flags}}
            TextureMapVersion::V3(tex) => {TextureFlags{inner: tex.header.flags}}
        }
    }

    pub fn texture_type(&self) -> TextureType {
        match &self.inner {
            TextureMapVersion::V1(tex) => {tex.header.type_}
            TextureMapVersion::V2(tex) => {tex.header.type_}
            TextureMapVersion::V3(tex) => {tex.header.type_}
        }
    }

    pub fn interpret_as(&self) -> Option<InterpretAs> {
        match &self.inner {
            TextureMapVersion::V1(tex) => {Some(tex.header.interpret_as)}
            TextureMapVersion::V2(_) => {None}
            TextureMapVersion::V3(tex) => {Some(tex.header.interpret_as)}
        }
    }

    pub fn dimensions(&self) -> Dimensions {
        match &self.inner {
            TextureMapVersion::V1(tex) => { tex.header.dimensions }
            TextureMapVersion::V2(_) => { Dimensions::_2D }
            TextureMapVersion::V3(tex) => { tex.header.dimensions }
        }
    }

    pub fn has_mipblock1(&self) -> bool {
        match &self.inner {
            TextureMapVersion::V1(t) => { t.has_mipblock_data() }
            TextureMapVersion::V2(t) => { t.has_mipblock_data() }
            TextureMapVersion::V3(t) => { t.has_mipblock_data() }
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P, woa_version: WoaVersion) -> Result<Self, TextureMapError> {
        let file = File::open(path).map_err(TextureMapError::IoError)?;
        let mut reader = BufReader::new(file);
        TextureMap::read_le_args(&mut reader, (woa_version,)).map_err(TextureMapError::ParsingError)
    }

    pub fn from_memory(data: &[u8], woa_version: WoaVersion) -> Result<Self, TextureMapError> {
        let mut reader = Cursor::new(data);
        TextureMap::read_le_args(&mut reader, (woa_version,)).map_err(TextureMapError::ParsingError)
    }

    pub fn default_mipmap(&self) -> Result<MipLevel, TextureMapError> {
        self.mipmap(self.default_mip_level() as usize)
    }

    pub fn mipmaps(&self) -> impl Iterator<Item = Result<MipLevel, TextureMapError>> + '_ {
        (0..self.num_mip_levels()).map(move |level| self.mipmap(level))
    }

    pub fn mipmap(&self, level: usize) -> Result<MipLevel, TextureMapError> {
        let removed_mip_count = self.texd_mip_levels() - self.text_mip_levels();

        let mut mips_sizes: Vec<u32> = self.mip_sizes();
        let mut block_sizes: Vec<u32> = self.compressed_mip_sizes();

        if !self.has_mipblock1() {
            let removed_mip = mips_sizes.drain(0..removed_mip_count).collect::<Vec<u32>>().pop().unwrap_or(0);
            mips_sizes.iter_mut().for_each(|x| if *x > 0 { *x -= removed_mip });

            let removed_block = block_sizes.drain(0..removed_mip_count).collect::<Vec<u32>>().pop().unwrap_or(0);
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
        let block = self.data().clone().into_iter().skip(block_start as usize).take(block_size as usize).collect::<Vec<u8>>();
        let data = if is_compressed {
            let mut dst = vec![0u8; mip_size as usize];
            match lz4::block::decompress_to_buffer(block.as_slice(), Some(mip_size as i32), &mut dst) {
                Ok(_) => {}
                Err(e) => {
                    return Err(TextureMapError::UnknownError(format!("Failed to decompress texture data {}", e)));
                }
            };
            dst
        } else {
            block
        };

        Ok(MipLevel {
            format: self.format(),
            width: self.width() >> level,
            height: self.height() >> level,
            data,
        })
    }

    pub fn has_atlas(&self) -> bool {
        self.atlas().is_some()
    }

    pub fn set_mipblock1(&mut self, mipblock: MipblockData){
        self.set_data(TextureData::Mipblock1(mipblock))
    }

    fn texd_header(&self) -> Result<Vec<u8>, TextureMapError>{
        let mut writer = Cursor::new(Vec::new());

        let data = match &self.inner{
            TextureMapVersion::V1(d) => {&d.data}
            TextureMapVersion::V2(d) => {&d.data}
            TextureMapVersion::V3(d) => {&d.data}
        };

        let atlas_size = self.atlas().as_ref().map(|atlas| atlas.size()).unwrap_or(0);
        let total_size = data.size()
            + match &self.inner {
            TextureMapVersion::V1(_) => {TextureMapHeaderV1::size()}
            TextureMapVersion::V2(_) => {TextureMapHeaderV2::size()}
            TextureMapVersion::V3(_) => {TextureMapHeaderV3::size()}
        }
            + atlas_size;

        let args = DynamicTextureMapArgs {
            data_size: total_size as u32,
            atlas_data_size: atlas_size as u32,

            //not needed as these are only used in H3, which doesn't use a texd header.
            text_scale: 0,
            text_mip_levels: 0,
        };

        match &self.inner {
            TextureMapVersion::V1(tex) => {tex.header.write_options(&mut writer, Endian::Little, (args,))?}
            TextureMapVersion::V2(tex) => {tex.header.write_options(&mut writer, Endian::Little, (args,))?}
            TextureMapVersion::V3(tex) => {tex.header.write_options(&mut writer, Endian::Little, (args,))?}
        }

        // If atlas_data is present, write it
        if let Some(atlas_data) = &self.atlas() {
            atlas_data.write_options(&mut writer, Endian::Little, ())?;
        }

        Ok(writer.into_inner())
    }


    pub fn pack_to_vec(&self) -> Result<Vec<u8>, TexturePackerError> {
        let mut writer = Cursor::new(Vec::new());
        self.pack_internal(&mut writer)?;
        Ok(writer.into_inner())
    }

    pub fn pack_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), TexturePackerError> {
        let file = fs::File::create(path).map_err(TexturePackerError::IoError)?;
        let mut writer = BufWriter::new(file);
        self.pack_internal(&mut writer)?;
        Ok(())
    }

    fn pack_internal<W: Write + Seek>(&self, writer: &mut W) -> Result<(), TexturePackerError> {
        self.write_le_args(writer, ())
            .map_err(TexturePackerError::SerializationError)?;
        Ok(())
    }
}

