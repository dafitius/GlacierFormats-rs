use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;
use binrw::{binread, BinRead, BinResult, BinWrite, Endian, NullString};
use bitfield_struct::bitfield;

#[binread]
#[br(little, repr = u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EFX2ShaderType {
    VertexShader = 0,
    PixelShader = 1,
    GeometryShader = 2,
    DomainShader = 3,
    HullShader = 4,
    ComputeShader = 5,
}


#[allow(redundant_semicolons)]
#[bitfield(u8)]
#[derive(BinRead, BinWrite, PartialEq, Eq)]
pub struct ShaderTags
{
    pub debug: bool,
    pub neo: bool,
    pub scorpio: bool,
    pub amd: bool,
    pub nvidia: bool,
    pub intel: bool,
    pub dx12: bool,
    __: bool,
}


/* Structs */

#[binread]
#[br(little)]
// #[br(assert(texture_states_offset == 0x33362626u32, "offset should be fixed"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2Header {
    pub num_shaders: u32,
    pub shaders_offset: u32,
    pub num_techniques: u32,
    pub techniques_offset: u32,
    pub texture_states_offset: u32,
}

#[binread]
#[br(little)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2ProgramHeader {
    pub magic_start: u32,
    pub name_offset: u32,
    pub program_type: EFX2ShaderType,
    pub tags: ShaderTags,
    pub program_offset: u32,
    pub program_size: u32,
    pub constant_desc_offset: u32,
    pub num_constants: u32,
    pub texture_desc_offset: u32,
    pub num_textures: u32,
    pub const_size: u32,
    pub const_buffer_bind_mask: u8,
    pub last_texture_stream: u8,
    pub _pad: [u8; 2],
    pub magic_end: u32,
}

#[binread]
#[br(little)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2ConstantDesc {
    pub name_offset: u32,
    pub _type: u32,
    pub offset: u32,
    pub size: u32,
}

/* SFX2TextureDesc is identical layout to SFX2ConstantDesc */
pub type SFX2TextureDesc = SFX2ConstantDesc;

#[binread]
#[br(little)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2TechniqueHeader {
    pub name_offset: u32,
    #[br(dbg)]
    pub num_passes: u32,
    #[br(dbg)]
    pub pass_start_offset: u32,
}

#[binread]
#[br(little)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2PassHeader {
    pub name_offset: u32,
    pub num_shaders: u32,
}

#[binread]
#[br(little)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2TextureStateHeader {
    pub name_offset: u32,
}


#[binread]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2Pass{
    #[br(temp)]
    test: u8,
}

#[binread]
#[br(assert(unk1 == 0x1, "unk1 should be 1"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamesHeader {
    magic: u32,
    offset: u32,
    unk1: u32,
    num_names: u32,
    names_offset: u32,
}

#[binread]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2Technique {
    #[br(temp)]
    pub header: SFX2TechniqueHeader,
    #[br(seek_before = SeekFrom::Start(header.name_offset as u64 + 0x10))]
    #[br(restore_position)]
    #[br(map(|nullstr: NullString| nullstr.to_string()))]
    pub name: String,

    #[br(seek_before = SeekFrom::Start(header.pass_start_offset as u64 + 0x10))]
    #[br(count(header.num_passes))]
    #[br(restore_position)]
    pub passes: Vec<SFX2Pass>
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2TextureStates(HashMap<String, String>);


impl BinRead for SFX2TextureStates {
    type Args<'a> = (&'a NamesHeader,);

    fn read_options<R: Read + Seek>(reader: &mut R, endian: Endian, args: Self::Args<'_>) -> BinResult<Self> {
        let header = args.0;
        reader.seek(SeekFrom::Start(header.names_offset as u64))?;
        reader.seek(SeekFrom::Current(0x10))?;


        let mut name_offsets = vec![];
        for _ in 0..header.num_names {
            name_offsets.push(u32::read_le(reader)?);
        }

        let mut read_name_at = |offset: u32| -> BinResult<String> {
            reader.seek(SeekFrom::Start(offset as u64))?;
            reader.seek(SeekFrom::Current(0x10))?;
            Ok(NullString::read_options(reader, endian, ())?.to_string())
        };

        let mut map = HashMap::new();
        let mut it = name_offsets.into_iter();
        while let (Some(k_off), Some(v_off)) = (it.next(), it.next()) {
            let key = read_name_at(k_off)?;
            let value = read_name_at(v_off)?;
            map.insert(key, value);
        }

        Ok(Self(map))
    }
}


#[binread]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SFX2Shader {
    #[br(temp)]
    pub effect_offset: u32,
    #[br(temp)]
    pub effect_size: u32,
    #[br(temp)]
    pub padd: u64,
    #[br(temp)]
    pub names: NamesHeader,
    pub header: SFX2Header,

    #[br(args(&names))]
    pub texture_states: SFX2TextureStates,
    #[br(seek_before(SeekFrom::Start(header.techniques_offset as u64 + 0x10)))]
    #[br(count(header.num_techniques))]
    pub techniques: Vec<SFX2Technique>,

}

pub fn hello_world() -> Result<(), std::io::Error> {
    println!("hello world");
    let folder: PathBuf = "/media/dafitius/980 PRO/HitmanProjects/Hitman files/ALL_MATE/H2016/all".into();
    for file in folder.read_dir()? {
        let file = file?;
        println!("START {:?}", file.path().file_name());
        let bytes = std::fs::read(file.path())?;
        let mut cursor = Cursor::new(&bytes);
        let sfx2 = SFX2Shader::read_le(&mut cursor).unwrap();
        println!("{:?}", sfx2);
    }

    Ok(())
}