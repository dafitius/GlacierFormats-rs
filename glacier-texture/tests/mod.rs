pub mod text_texd;
use std::fs;
use std::path::Path;

pub fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    fs::read(path.join(name)).expect("cannot read fixture")
}