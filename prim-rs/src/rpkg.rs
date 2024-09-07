use std::io::Cursor;
use rpkg_rs::{GlacierResource, GlacierResourceError, WoaVersion};
use crate::render_primitive::RenderPrimitive;

impl GlacierResource for RenderPrimitive {
    type Output = RenderPrimitive;

    fn process_data<R: AsRef<[u8]>>(_: rpkg_rs::WoaVersion, data: R) -> Result<Self::Output, GlacierResourceError> {
        let mut stream = Cursor::new(data);
        RenderPrimitive::parse_bytes(&mut stream).map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(&self, woa_version: WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        todo!()
    }

    fn resource_type(&self) -> [u8; 4] {
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
}