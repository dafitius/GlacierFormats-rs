<div align="center">
  <h1><code>GlacierFormats-rs</code></h1>
</div>

`GlacierFormats-rs` provides functionality for interacting with various Glacier 2 file formats. 
The crates in this workspace facilitate reading and writing of these formats.

**Note:** This project is currently in alpha stage. Expect changes to the API.

## Features

#### Supported File Formats:
- RenderPrimitive ([PRIM](prim-rs)) files, containing mesh, hitbox and cloth data. [experimental]
- TextureMap and MipblockData ([TEXT and TEXD](tex-rs)) files, containg texture data.

#### Optional rpkg-rs support
`rpkg-rs` aims to streamline the process of working with Hitman game resources, offering a robust set of features to read ResourcePackage files.
All formats in this workspace optionally implement the GlacierResource trait, ensuring easy integration into [rpkg-rs](https://github.com/dafitius/rpkg-rs)

example rpkg-rs code using the TextureMap resource
```rust
let rrid = ResourceID::from_string("[assembly:/_pro/_test/dafits/textures/rocco_a.texture?/diffuse_a.tex](ascolormap).pc_tex").to_rrid();
// export through a templated function
let texture = partition.get_resource::<TextureMap>(WoaVersion::HM3, rrid)?;
println!("({}x{})", texture.width(), texture.height());

// or with a custom function, adding additional functionality
tex_rs::get_full_texture(partition_manager, WoaVersion::HM3, rrid)?;
println!("({}x{})", texture.width(), texture.height());
```

## Contributions
Bug reports, PRs and feature requests are welcome.

## License
This project is licensed under the Apache 2.0 License - see the LICENSE.md file for details.
