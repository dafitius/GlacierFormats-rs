use std::io::Cursor;
use rpkg_rs::{GlacierResource, GlacierResourceError};
use crate::render_primitive::RenderPrimitive;

impl GlacierResource for RenderPrimitive {
    type Output = RenderPrimitive;

    fn process_data<R: AsRef<[u8]>>(_: rpkg_rs::WoaVersion, data: R) -> Result<Self::Output, GlacierResourceError> {
        let mut stream = Cursor::new(data);
        RenderPrimitive::parse_bytes(&mut stream).map_err(|e| GlacierResourceError::ReadError(e.to_string()))
    }

    fn serialize(resource: &Self::Output, woa_version: rpkg_rs::WoaVersion) -> Result<Vec<u8>, GlacierResourceError> {
        todo!()
    }

    fn get_video_memory_requirement(resource: &Self::Output) -> u64 {
        todo!()
    }

    fn get_system_memory_requirement(resource: &Self::Output) -> u64 {
        todo!()
    }
}