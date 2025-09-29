use crate::render_primitive::RenderPrimitive;
use rpkg_rs::{GlacierResource, GlacierResourceError};
use std::io::Cursor;
use crate::rig::bone_rig::BoneRig;
use crate::WoaVersion;

impl From<crate::WoaVersion> for rpkg_rs::WoaVersion{
    fn from(value: WoaVersion) -> Self {
        match value{
            WoaVersion::HM2016 => Self::HM2016,
            WoaVersion::HM2 => Self::HM2,
            WoaVersion::HM3 => Self::HM3
        }
    }
}

impl From<rpkg_rs::WoaVersion> for WoaVersion{
    fn from(value: rpkg_rs::WoaVersion) -> Self {
        match value {
            rpkg_rs::WoaVersion::HM2016 => Self::HM2016,
            rpkg_rs::WoaVersion::HM2 => Self::HM2,
            rpkg_rs::WoaVersion::HM3 => Self::HM3,
        }
    }
}

impl GlacierResource for RenderPrimitive {
    type Output = RenderPrimitive;

    fn process_data<R: AsRef<[u8]>>(
        woa_version : rpkg_rs::WoaVersion,
        data: R,
    ) -> Result<Self::Output, GlacierResourceError> {
        let mut stream = Cursor::new(data);
        RenderPrimitive::parse_bytes(&mut stream, woa_version.into())
            .map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(&self, _: rpkg_rs::WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        todo!()
    }

    fn resource_type() -> [u8; 4] {
        todo!()
    }

    fn video_memory_requirement(&self) -> u64 {
        todo!()
    }

    fn system_memory_requirement(&self) -> u64 {
        todo!()
    }

    fn should_scramble(&self) -> bool {
        todo!()
    }

    fn should_compress(&self) -> bool {
        todo!()
    }
}


impl GlacierResource for BoneRig {
    type Output = BoneRig;

    fn process_data<R: AsRef<[u8]>>(
        _: rpkg_rs::WoaVersion,
        data: R,
    ) -> Result<Self::Output, GlacierResourceError> {
        BoneRig::from_memory(data.as_ref())
            .map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(&self, _: rpkg_rs::WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        self.pack_to_vec().map_err(|e| GlacierResourceError::WriteError(e.to_string()))
    }

    fn resource_type() -> [u8; 4] {
        *b"BORG"
    }

    fn video_memory_requirement(&self) -> u64 {
        0xFFFFFFFF
    }

    fn system_memory_requirement(&self) -> u64 {
        0xFFFFFFFF
    }

    fn should_scramble(&self) -> bool {
        true
    }

    fn should_compress(&self) -> bool {
        true
    }
}
