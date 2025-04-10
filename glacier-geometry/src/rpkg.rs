use crate::render_primitive::RenderPrimitive;
use rpkg_rs::{GlacierResource, GlacierResourceError, WoaVersion};
use std::io::Cursor;
use crate::rig::bone_rig::BoneRig;

impl GlacierResource for RenderPrimitive {
    type Output = RenderPrimitive;

    fn process_data<R: AsRef<[u8]>>(
        _: rpkg_rs::WoaVersion,
        data: R,
    ) -> Result<Self::Output, GlacierResourceError> {
        let mut stream = Cursor::new(data);
        RenderPrimitive::parse_bytes(&mut stream)
            .map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(&self, woa_version: WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
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
        _: WoaVersion,
        data: R,
    ) -> Result<Self::Output, GlacierResourceError> {
        BoneRig::from_memory(data.as_ref())
            .map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(&self, _: WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
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
