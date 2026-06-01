use binrw::{BinRead, BinWrite};
use bitfield_struct::bitfield;
use directxtex::{DXGI_FORMAT, TEX_DIMENSION};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[brw(repr = u16)]
pub enum TextureType
{
    Colour = 0,
    #[default]
    Normal = 1,
    Height = 2,
    CompoundNormal = 3,
    Billboard = 4,
    UNKNOWN5 = 5, //introduced in knt
    Projection = 6,
    Emission = 16,
    //UNKNOWN64 = 64, //unused

    //UNKNOWN128 = 128, //unused in H2, H3
    Cubemap = 256, //uses ascolormap and ascubemap
    UNKNOWN512 = 512, //asheightmap
    UNKNOWN517 = 517, //introduced in knt
    //UNKNOWN1024 = 1024, //unused
}

#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[brw(repr = u8)]
pub enum InterpretAs
{
    Colour = 0,
    #[default]
    Normal = 1,
    Height = 2,
    CompoundNormal = 3,
    Billboard = 4,
    Cubemap = 6,
    Emission = 16, //This is an assumption
    Volume = 64, //This as well
}


#[derive(Serialize, Deserialize, Debug, Copy)]
#[derive(Clone, PartialEq, Hash, Eq)]
pub enum RenderFormat
{
    R32G32B32A32, //idk
    R16G16B16A16,
    R8G8B8A8,
    R32,
    R8G8,
    A8,
    BC1,
    BC2,
    BC3,
    BC4,
    BC5,
    BC7,
}

impl RenderFormat {
    pub fn is_compressed(&self) -> bool {
        matches!(self, RenderFormat::BC1|
            RenderFormat::BC2|
            RenderFormat::BC3|
            RenderFormat::BC4|
            RenderFormat::BC5|
            RenderFormat::BC7)
    }

    pub fn num_channels(&self) -> usize {
        match self {
            RenderFormat::A8 | RenderFormat::R32 | RenderFormat::BC4 => 1,
            RenderFormat::R8G8 | RenderFormat::BC5 => 2,
            RenderFormat::BC1 | //assume DXT1a
            RenderFormat::R32G32B32A32 |
            RenderFormat::R16G16B16A16 |
            RenderFormat::R8G8B8A8 |
            RenderFormat::BC2 |
            RenderFormat::BC3 |
            RenderFormat::BC7 => 4,
        }
    }

    pub fn decompressed_format(&self) -> RenderFormat{
        match self{
            RenderFormat::BC1 => {RenderFormat::R8G8B8A8 }
            RenderFormat::BC2 => {RenderFormat::R8G8B8A8}
            RenderFormat::BC3 => {RenderFormat::R8G8B8A8}
            RenderFormat::BC4 => {RenderFormat::A8}
            RenderFormat::BC5 => {RenderFormat::R8G8}
            RenderFormat::BC7 => {RenderFormat::R8G8B8A8}
            format => *format
        }
    }
}

impl From<RenderFormat> for DXGI_FORMAT {
    fn from(value: RenderFormat) -> Self {
        match value {
            RenderFormat::R32G32B32A32 => { DXGI_FORMAT::DXGI_FORMAT_R32G32B32A32_FLOAT }
            RenderFormat::R16G16B16A16 => { DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_FLOAT } // has to be float
            RenderFormat::R8G8B8A8 => { DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM }
            RenderFormat::R32 => {DXGI_FORMAT::DXGI_FORMAT_R32_FLOAT }
            RenderFormat::R8G8 => { DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM }
            RenderFormat::A8 => { DXGI_FORMAT::DXGI_FORMAT_A8_UNORM }
            RenderFormat::BC1 => { DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM }
            RenderFormat::BC2 => { DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM }
            RenderFormat::BC3 => { DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM }
            RenderFormat::BC4 => { DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM }
            RenderFormat::BC5 => { DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM }
            RenderFormat::BC7 => { DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM }
        }
    }
}

pub trait RenderFormatMapping {
    const MAP: &'static [(u16, RenderFormat)];

    fn try_from_u16(raw: u16) -> Option<RenderFormat> {
        Self::MAP
            .iter()
            .find(|(r, _)| *r == raw)
            .map(|(_, f)| *f)
    }

    fn try_to_u16(format: RenderFormat) -> Option<u16> {
        Self::MAP
            .iter()
            .find(|(_, f)| *f == format)
            .map(|(r, _)| *r)
    }
}

#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy)]
#[derive(Clone, PartialEq, Hash, Eq)]
pub struct WoaRenderFormat {
    #[br(try_map = |raw: u16| Self::try_from_u16(raw).ok_or("invalid WOA render format"))]
    #[bw(map = |fmt: &RenderFormat| Self::try_to_u16(*fmt).expect("unsupported WOA render format"))]
    pub(crate) format: RenderFormat,
}

impl RenderFormatMapping for WoaRenderFormat{
    const MAP: &'static [(u16, RenderFormat)] = &[
        (0x02, RenderFormat::R32G32B32A32),
        (0x0A, RenderFormat::R16G16B16A16),
        (0x1C, RenderFormat::R8G8B8A8),
        (0x34, RenderFormat::R8G8),
        (0x42, RenderFormat::A8),
        (0x49, RenderFormat::BC1),
        (0x4C, RenderFormat::BC2),
        (0x4F, RenderFormat::BC3),
        (0x52, RenderFormat::BC4),
        (0x55, RenderFormat::BC5),
        (0x5A, RenderFormat::BC7)
    ];
}

impl From<WoaRenderFormat> for RenderFormat {
    fn from(format: WoaRenderFormat) -> Self {
        format.format
    }
}

impl PartialEq<RenderFormat> for WoaRenderFormat {
    fn eq(&self, other: &RenderFormat) -> bool {
        self.format == *other
    }
}

impl PartialEq<WoaRenderFormat> for RenderFormat {
    fn eq(&self, other: &WoaRenderFormat) -> bool {
        *self == other.format
    }
}


#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy)]
#[derive(Clone, PartialEq, Hash, Eq)]
pub struct BondRenderFormat {
    #[br(try_map = |raw: u16| Self::try_from_u16(raw).ok_or(format!("invalid Bond render format {}", raw)))]
    #[bw(map = |fmt: &RenderFormat| Self::try_to_u16(*fmt).expect("unsupported Bond render format"))]
    format: RenderFormat,
}

impl RenderFormatMapping for BondRenderFormat{
    const MAP: &'static [(u16, RenderFormat)] = &[
        (0x02, RenderFormat::R32G32B32A32),
        (0x0A, RenderFormat::R16G16B16A16),
        (0x1C, RenderFormat::R8G8B8A8),
        (0x2C, RenderFormat::R32),
        (0x34 + 3, RenderFormat::R8G8),
        (0x42 + 3, RenderFormat::A8),
        (0x49 + 3, RenderFormat::BC1),
        (0x4C + 3 , RenderFormat::BC2),
        (0x4F + 3, RenderFormat::BC3),
        (0x52 + 3, RenderFormat::BC4),
        (0x55 + 3, RenderFormat::BC5),
        (0x5A + 4, RenderFormat::BC7)
    ];
}

impl From<BondRenderFormat> for RenderFormat {
    fn from(format: BondRenderFormat) -> Self {
        format.format
    }
}

impl PartialEq<RenderFormat> for BondRenderFormat {
    fn eq(&self, other: &RenderFormat) -> bool {
        self.format == *other
    }
}

impl PartialEq<BondRenderFormat> for RenderFormat {
    fn eq(&self, other: &BondRenderFormat) -> bool {
        *self == other.format
    }
}


#[derive(Debug, Error)]
#[error("Unsupported DXGI_FORMAT")]
pub struct UnsupportedFormatError;

impl TryFrom<DXGI_FORMAT> for RenderFormat {
    type Error = UnsupportedFormatError;

    fn try_from(value: DXGI_FORMAT) -> Result<Self, Self::Error> {
        match value {
            DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_UNORM => Ok(RenderFormat::R16G16B16A16),
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM => Ok(RenderFormat::R8G8B8A8),
            DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM => Ok(RenderFormat::R8G8),
            DXGI_FORMAT::DXGI_FORMAT_A8_UNORM => Ok(RenderFormat::A8),
            DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM => Ok(RenderFormat::BC1),
            DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM => Ok(RenderFormat::BC2),
            DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM => Ok(RenderFormat::BC3),
            DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM => Ok(RenderFormat::BC4),
            DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM => Ok(RenderFormat::BC5),
            DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM => Ok(RenderFormat::BC7),
            _ => Err(UnsupportedFormatError),
        }
    }
}

#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[brw(repr = u8)]
pub enum Dimensions
{
    #[default]
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
#[derive(BinRead, BinWrite, Serialize, Deserialize)]
//#[brw(repr = u32)]
//most of these are unused...
pub(crate) struct TextureFlagsInner
{
    /* 0x1 */ pub(crate) swizzled: bool,
    /* 0x2 */ pub(crate) deferred: bool,           //Only used on 4x4 textures
    /* 0x4 */ pub(crate) memory_read_xbox_360: bool,
    /* 0x8 */ pub(crate) unknown1: bool,           //Does not affect the texture in-game. Usually not enabled on non-normal/color types, or uncompressed formats
    /* 0x10 */pub(crate) atlas: bool,              //Only used on atlas textures
    /* 0x20 */pub(crate) ddsc_encoded: bool,
    /* 0x40 */pub(crate) unknown3: bool,           //Not enabling this will corrupt most textures

    #[bits(25)]
    __: u32,
}

pub struct TextureFlags{
    ///inner "real" flags bitfield. Wrapped because it is likely to change over time.
    /// Wrapping the struct makes it possible to #\[deprecated\] old getters and setters
    pub(crate) inner: TextureFlagsInner
}

/// Flags set in the texture files.
/// The "unstable" flags are not found in any production texture file. Use at your own risk
/// The other flags should not crash the game, but can also result in corrupted textures, use with caution
impl TextureFlags{

    pub fn deferred(&self) -> bool { self.inner.deferred() }
    pub fn unknown1(&self) -> bool { self.inner.unknown1() }
    pub fn atlas(&self) -> bool { self.inner.atlas() }
    pub fn unknown3(&self) -> bool { self.inner.unknown3() }

    pub fn set_deferred(&mut self, value: bool) {
        self.inner.set_deferred(value)
    }
    pub fn set_unknown1(&mut self, value: bool) {
        self.inner.set_unknown1(value)
    }
    pub fn set_unknown3(&mut self, value: bool) {
        self.inner.set_unknown3(value)
    }

    pub fn set_atlas(&mut self, value: bool) {
    self.inner.set_atlas(value)
    }
    pub fn with_deferred(&self, value: bool) -> Self{
        Self{
            inner: self.inner.with_deferred(value)
        }
    }
    pub fn with_unknown1(&mut self, value: bool) -> Self{
        Self{
            inner: self.inner.with_unknown1(value)
        }
    }
    pub fn with_atlas(&mut self, value: bool) -> Self{
        Self{
            inner: self.inner.with_atlas(value)
        }
    }
    pub fn with_unknown3(&mut self, value: bool) -> Self{
        Self{
            inner: self.inner.with_unknown3(value)
        }
    }

    #[cfg(feature = "unstable")]
    pub fn swizzled(&self) -> bool { self.inner.swizzled() }

    #[cfg(feature = "unstable")]
    pub fn memory_read_xbox_360(&self) -> bool { self.inner.memory_read_xbox_360() }

    #[cfg(feature = "unstable")]
    pub fn ddsc_encoded(&self) -> bool { self.inner.ddsc_encoded() }

    #[cfg(feature = "unstable")]
    pub fn set_swizzled(&mut self, value: bool) {
        self.inner.set_swizzled(value)
    }

    #[cfg(feature = "unstable")]
    pub fn set_memory_read_xbox_360(&mut self, value: bool) {
        self.inner.set_memory_read_xbox_360(value)
    }

    #[cfg(feature = "unstable")]
    pub fn set_ddsc_encoded(&mut self, value: bool) {
        self.inner.set_ddsc_encoded(value)
    }

    #[cfg(feature = "unstable")]
    pub fn with_swizzled(&self, value: bool) -> Self {
        Self {
            inner: self.inner.with_swizzled(value),
        }
    }

    #[cfg(feature = "unstable")]
    pub fn with_memory_read_xbox_360(&self, value: bool) -> Self {
        Self {
            inner: self.inner.with_memory_read_xbox_360(value),
        }
    }

    #[cfg(feature = "unstable")]
    pub fn with_ddsc_encoded(&self, value: bool) -> Self {
        Self {
            inner: self.inner.with_ddsc_encoded(value),
        }
    }
}