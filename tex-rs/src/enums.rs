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
    UNKNOWN64 = 64,

    UNKNOWN128 = 128,
    UNKNOWN256 = 256,
    UNKNOWN512 = 512,
    UNKNOWN1024 = 1024,
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
    Projection = 6,
    Emission = 16,
    UNKNOWN64 = 64,
}


#[derive(BinRead, BinWrite, Serialize, Deserialize, Debug, Copy)]
#[brw(repr = u16)]
#[derive(Clone, PartialEq, Hash, Eq)]
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

impl From<DXGI_FORMAT> for RenderFormat {
    fn from(value: DXGI_FORMAT) -> Self {
        match value {
            DXGI_FORMAT::DXGI_FORMAT_R16G16B16A16_UNORM => RenderFormat::R16G16B16A16,
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM => RenderFormat::R8G8B8A8,
            DXGI_FORMAT::DXGI_FORMAT_R8G8_UNORM => RenderFormat::R8G8,
            DXGI_FORMAT::DXGI_FORMAT_A8_UNORM => RenderFormat::A8,
            DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM => RenderFormat::DXT1,
            DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM => RenderFormat::DXT3,
            DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM => RenderFormat::DXT5,
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
pub struct RenderResourceMiscFlags
{
    persistent_data: bool,
    pub texture_cube: bool,
    texture_normalmap: bool,
    pub texture_swizzled: bool,
    pub temp_alloc: bool,
    unused2: bool,
    pub no_color_compression: bool,
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
