use std::io::Cursor;
use binrw::BinRead;
use rpkg_rs::{GlacierResource, GlacierResourceError};
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

impl GlacierResource for TextureMap {
    type Output = TextureMap;

    fn process_data<R: AsRef<[u8]>>(woa_version: rpkg_rs::WoaVersion, data: R) -> Result<Self::Output, GlacierResourceError> {
        let mut stream = Cursor::new(data);
        TextureMap::read_le_args(&mut stream, (WoaVersion::from(woa_version), )).map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(resource: &Self::Output, woa_version: rpkg_rs::WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        todo!()
    }

    fn video_memory_requirement(resource: &Self::Output) -> u64 {
        todo!()
    }

    fn system_memory_requirement(resource: &Self::Output) -> u64 {
        todo!()
    }
}

pub fn full_texture(manager: &rpkg_rs::resource::partition_manager::PartitionManager, woa_version: rpkg_rs::WoaVersion, rrid: rpkg_rs::resource::runtime_resource_id::RuntimeResourceID) -> Result<TextureMap, GlacierResourceError> {
    let res_info = manager.resource_info_from(&"chunk0".parse().unwrap(), &rrid).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;
    let data = manager.read_resource_from("chunk0".parse().unwrap(), rrid).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;

    let mut stream = Cursor::new(data);
    let mut texture_map = TextureMap::read_le_args(&mut stream, (WoaVersion::from(woa_version), )).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;

    if let Some((rrid, flag)) = res_info.references().get(0){
        let texd_data = manager.read_resource_from("chunk0".parse().unwrap(), *rrid).map_err(|e| GlacierResourceError::ReadError(format!("Tried to load broken depend: {}", e)))?;
        texture_map.set_mipblock1_data(&texd_data, WoaVersion::from(woa_version)).map_err(|e| GlacierResourceError::ReadError(e.to_string()))?;
    }
    Ok(texture_map)
}