use std::io::Cursor;
use binrw::{BinRead, BinWrite};
use rpkg_rs::{GlacierResource, GlacierResourceError};
use crate::mipblock::MipblockData;
use crate::pack::TexturePackerError;
use crate::texture_map::TextureMap;
use crate::WoaVersion;


impl From<rpkg_rs::WoaVersion> for WoaVersion {
    fn from(value: rpkg_rs::WoaVersion) -> Self {
        match value {
            rpkg_rs::WoaVersion::HM2016 => { WoaVersion::HM2016 }
            rpkg_rs::WoaVersion::HM2 => { WoaVersion::HM2 }
            rpkg_rs::WoaVersion::HM3 => { WoaVersion::HM3 }
        }
    }
}

impl From<WoaVersion> for rpkg_rs::WoaVersion{
    fn from(value: WoaVersion) -> Self {
        match value {
            WoaVersion::HM2016 => { rpkg_rs::WoaVersion::HM2016 }
            WoaVersion::HM2 => { rpkg_rs::WoaVersion::HM2 }
            WoaVersion::HM3 => { rpkg_rs::WoaVersion::HM3 }
        }
    }
}

impl GlacierResource for TextureMap {
    type Output = TextureMap;

    fn process_data<R: AsRef<[u8]>>(woa_version: rpkg_rs::WoaVersion, data: R) -> Result<Self::Output, GlacierResourceError> {
        let mut stream = Cursor::new(data);
        TextureMap::read_le_args(&mut stream, (WoaVersion::from(woa_version), )).map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(&self, _: rpkg_rs::WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        //TODO: woa version gets ignored currently. Getting the packer to accept TextureMap would allow for easy porting.
        let mut writer = Cursor::new(Vec::new());
        self.write_le_args(&mut writer, ())
            .map_err(TexturePackerError::SerializationError).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?; //TODO: change this
        Ok(writer.into_inner())
    }

    fn resource_type(&self) -> [u8; 4] {
        *b"TEXT"
    }

    fn video_memory_requirement(&self) -> u64 {
        self.video_memory_requirement() as u64
    }

    fn system_memory_requirement(&self) -> u64 {
        0xFFFFFFFF
    }

    fn should_scramble(&self) -> bool {
        true
    }

    fn should_compress(&self) -> bool {
        match self.version(){
            WoaVersion::HM2016 => {true}
            WoaVersion::HM2 |
            WoaVersion::HM3 => {false}
        }
    }
}

impl GlacierResource for MipblockData{
    type Output = MipblockData;

    fn process_data<R: AsRef<[u8]>>(woa_version: rpkg_rs::WoaVersion, data: R) -> Result<Self::Output, GlacierResourceError> {
        let mipblock = MipblockData::new(&data.as_ref().to_vec(), woa_version.into()).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;
        Ok(mipblock)
    }

    fn serialize(&self, woa_version: rpkg_rs::WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        if self.header.is_empty() && (woa_version == rpkg_rs::WoaVersion::HM2016 || woa_version == rpkg_rs::WoaVersion::HM2) {
            return Err(GlacierResourceError::ReadError(format!("Cannot serialize to {:?} without header data :(", woa_version)));
        }
        self.pack_to_vec(woa_version.into()).map_err( |e| GlacierResourceError::WriteError(format!("Texd packing error: {}", e)))
    }

    fn resource_type(&self) -> [u8; 4] {
        *b"TEXD"
    }

    fn video_memory_requirement(&self) -> u64 {
        self.video_memory_requirement() as u64
    }

    fn system_memory_requirement(&self) -> u64 {
        0xFFFFFFFF
    }

    fn should_scramble(&self) -> bool {
        false
    }

    fn should_compress(&self) -> bool {
        false
    }
}

pub fn full_texture(manager: &rpkg_rs::resource::partition_manager::PartitionManager, woa_version: rpkg_rs::WoaVersion, rrid: rpkg_rs::resource::runtime_resource_id::RuntimeResourceID) -> Result<TextureMap, GlacierResourceError> {
    let res_info = manager.resource_info_from(&"chunk0".parse().unwrap(), &rrid).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;
    let data = manager.read_resource_from("chunk0".parse().unwrap(), rrid).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;

    let mut stream = Cursor::new(data);
    let mut texture_map = TextureMap::read_le_args(&mut stream, (WoaVersion::from(woa_version), )).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;

    if let Some((rrid, _)) = res_info.references().first(){
        let texd_data = manager.read_resource_from("chunk0".parse().unwrap(), *rrid).map_err(|e| GlacierResourceError::ReadError(format!("Tried to load broken depend: {}", e)))?;
        texture_map.set_mipblock1_raw(&texd_data, WoaVersion::from(woa_version)).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;
    }
    Ok(texture_map)
}