use std::fs;
use std::io::{BufWriter, Cursor, Read, Seek, Write};
use std::path::Path;
use binrw::{BinRead, BinReaderExt};
use serde::{Deserialize, Serialize};
use crate::atlas::AtlasData;
use crate::pack::TexturePackerError;
use crate::texture_map::{TextureMapError, TextureMapHeaderImpl, TextureMapHeaderV1, TextureMapHeaderV2};
use crate::WoaVersion;
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MipblockData {
    pub video_memory_requirement: usize,
    pub header: Vec<u8>,
    pub data: Vec<u8>,
}

impl From<MipblockData> for Vec<u8> {
    fn from(value: MipblockData) -> Self {
        value.data
    }
}

impl MipblockData{

    pub fn from_file<P: AsRef<Path>>(path: P, woa_version: WoaVersion) -> Result<Self, TextureMapError> {
        let data = fs::read(path).map_err(TextureMapError::IoError)?;
        Self::new_inner(&data, woa_version)
    }

    pub fn from_memory(data: &[u8], woa_version: WoaVersion) -> Result<Self, TextureMapError> {
        Self::new_inner(data, woa_version)
    }

    fn new_inner(data: &[u8], version: WoaVersion) -> Result<Self, TextureMapError>{
        let mut stream = Cursor::new(data);
        let mut header = vec![];
        let mut memory_reqs = 0;

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

                header = vec![0u8; stream.position() as usize];
                stream.set_position(0);
                stream.read_exact(&mut header).map_err(TextureMapError::IoError)?;

                memory_reqs = (texd_header.mip_sizes.first().copied().unwrap_or(0x0) + texd_header.mip_sizes.get(1).copied().unwrap_or(0x0)) as usize;

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

                header = vec![0u8; stream.position() as usize];
                stream.set_position(0);
                stream.read_exact(&mut header).map_err(TextureMapError::IoError)?;

                memory_reqs = (texd_header.mip_sizes.first().copied().unwrap_or(0x0) + texd_header.mip_sizes.get(1).copied().unwrap_or(0x0)) as usize;

                data_size as usize - (TextureMapHeaderV2::size()) - atlas.map(|a| a.size()).unwrap_or(0)
            }
            WoaVersion::HM3 => {
                data.len()
            }
        };

        let mut buffer = vec![0u8; read_size];
        stream.read_exact(&mut buffer)?;
        Ok(Self{
            video_memory_requirement: memory_reqs,
            header,
            data: buffer,
        })
    }

    pub fn video_memory_requirement(&self) -> usize{
        self.video_memory_requirement
    }

    pub fn pack_to_vec(&self, woa_version: WoaVersion) -> Result<Vec<u8>, TexturePackerError> {
        let mut writer = Cursor::new(Vec::new());
        self.pack_internal(&mut writer, woa_version)?;
        Ok(writer.into_inner())
    }

    pub fn pack_to_file<P: AsRef<Path>>(&self, path: P, woa_version: WoaVersion) -> Result<(), TexturePackerError> {
        let file = fs::File::create(path).map_err(TexturePackerError::IoError)?;
        let mut writer = BufWriter::new(file);
        self.pack_internal(&mut writer, woa_version)?;
        Ok(())
    }

    fn pack_internal<W: Write + Seek>(&self, writer: &mut W, woa_version: WoaVersion) -> Result<(), TexturePackerError> {
        writer.write_all(match woa_version{
            WoaVersion::HM2016 |
            WoaVersion::HM2 => {
                self.header.iter().chain(&self.data).cloned().collect::<Vec<u8>>()
            }
            WoaVersion::HM3 => {
                self.data.clone()
            }
        }.as_slice()).map_err(|e| TexturePackerError::PackingError(format!("Unable to pack mipblock1: {e}")))?;
        Ok(())
    }
}