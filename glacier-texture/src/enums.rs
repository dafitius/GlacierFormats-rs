use binrw::{BinRead, BinWrite};
use bitfield_struct::bitfield;
use directxtex::{DXGI_FORMAT, TEX_DIMENSION};
use serde::{Deserialize, Serialize};

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
    Projection = 6,
    Emission = 16,
    //UNKNOWN64 = 64, //unused

    //UNKNOWN128 = 128, //unused in H2, H3
    Cubemap = 256, //uses ascolormap and ascubemap
    UNKNOWN512 = 512, //asheightmap
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


#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy)]
#[brw(repr = u16)]
#[derive(Clone, PartialEq, Hash, Eq)]
pub enum RenderFormat
{
    R16G16B16A16 = 0x0A,
    R8G8B8A8 = 0x1C,
    R8G8 = 0x34,
    A8 = 0x42,
    BC1 = 0x49,
    BC2 = 0x4C,
    BC3 = 0x4F,
    BC4 = 0x52,
    BC5 = 0x55,
    BC7 = 0x5A,
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
            RenderFormat::A8 | RenderFormat::BC4 => 1,
            RenderFormat::R8G8 | RenderFormat::BC5 => 2,
            RenderFormat::BC1 | //assume DXT1a
            RenderFormat::R16G16B16A16 |
            RenderFormat::R8G8B8A8 |
            RenderFormat::BC2 |
            RenderFormat::BC3 |
            RenderFormat::BC7 => 4,
        }
    }
}

impl From<RenderFormat> for DXGI_FORMAT {
    fn from(value: RenderFormat) -> Self {
        match value {
            RenderFormat::R16G16B16A16 => { DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_FLOAT } // has to be float
            RenderFormat::R8G8B8A8 => { DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM }
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

impl From<DXGI_FORMAT> for RenderFormat {
    fn from(value: DXGI_FORMAT) -> Self {
        match value {
            DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_UNORM => RenderFormat::R16G16B16A16,
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM => RenderFormat::R8G8B8A8,
            DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM => RenderFormat::R8G8,
            DXGI_FORMAT::DXGI_FORMAT_A8_UNORM => RenderFormat::A8,
            DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM => RenderFormat::BC1,
            DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM => RenderFormat::BC2,
            DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM => RenderFormat::BC3,
            DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM => RenderFormat::BC4,
            DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM => RenderFormat::BC5,
            DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM => RenderFormat::BC7,
            _ => panic!("Unsupported DXGI_FORMAT: {:?}", value),
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