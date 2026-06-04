pub mod text_texd;
//pub mod box_reflection; //disabled for performance reasons
use std::fs;
use std::path::Path;

pub fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    fs::read(path.join(name)).expect(format!("cannot read fixture at {}", path.join(name).display()).as_str())
}

pub fn write_fixture<C: AsRef<[u8]>>(name: &str, contents: C) {

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    println!("writing fixture: {}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|_| panic!("cannot create fixture directory at {}", parent.display()));
    }

    fs::write(&path, contents)
        .unwrap_or_else(|_| panic!("cannot write fixture at {}", path.display()));
}