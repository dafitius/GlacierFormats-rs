use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use binrw::BinRead;
use clap::{Args, Parser, Subcommand};
use tex_rs::convert;
use tex_rs::mipblock::MipblockData;
use tex_rs::pack::TextureMapBuilder;
use tex_rs::texture_map::TextureMap;
use tex_rs::WoaVersion;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(flatten)]
    global_opts: GlobalOpts,

    /// Command to execute
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Convert a TEXT file to a DDS file
    ConvertTextureMapToDDS(ConvertTextureMap),

    /// Convert a TEXT file to a TGA file
    ConvertTextureMapToTga(ConvertTextureMap),

    /// Convert a TGA file to a TEXT (and TEXD) file
    ConvertTgaToTextureMap(GenerateTextureMap),

    /// Port a TEXT file from one game version to another
    PortTextureMap(PortTextureMap),
}

#[derive(Debug, Args)]
struct GlobalOpts {
    /// The directory to output to, if there's any
    #[clap(short, long, global = true)]
    output_path: Option<String>,

    /// When this flag is set, only errors will be shown
    #[clap(short, long, global = true)]
    silent: bool,
}

#[derive(Debug, Args)]
#[command(author, version, about, long_about = None)]
struct ConvertTextureMap {
    /// Version of the game you want to convert from options: [HM2016, HM2, HM3]
    #[clap(short, long)]
    game_version: WoaVersion,

    /// Path to the .text file
    #[arg(short, long)]
    input_path: String,

    /// Path to a .texd file
    #[clap(short = 'd', long)]
    texd_path: Option<String>,
}

#[derive(Debug, Args)]
#[command(author, version, about, long_about = None)]
struct GenerateTextureMap {
    /// Version of the game you want to generate for, options: [HM2016, HM2, HM3]
    #[clap(short, long)]
    game_version: WoaVersion,

    /// Path to the input file
    #[arg(short, long)]
    input_path: String,

    /// Enable this to generate only a .text file
    #[arg(long)]
    no_texd: bool,
}

#[derive(Debug, Args)]
#[command(author, version, about, long_about = None)]
struct PortTextureMap {
    /// Version of the game you want to port from, options: [HM2016, HM2, HM3]
    #[clap(short, long)]
    from_version: WoaVersion,

    /// Version of the game you want to port to, options: [HM2016, HM2, HM3]
    #[clap(short, long)]
    to_version: WoaVersion,

    /// Path to the input .text file
    #[arg(short, long)]
    input_path: String,

    /// Path to a .texd file
    #[clap(short = 'd', long)]
    texd_path: Option<String>,

    /// Enable this to generate only a .text file
    #[arg(long)]
    no_texd: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::ConvertTextureMapToDDS(cmd) => {
            let tex = read_texture(&cmd, cli.global_opts.silent)?;

            let output_path = get_output_path(&cli.global_opts.output_path, &cmd.input_path, "dds");

            let dds = convert::create_dds(&tex)
                .context("Failed to create DDS from the texture map")?;
            fs::write(&output_path, dds)
                .with_context(|| format!("Failed to write DDS file to {:?}", output_path))?;

            if !cli.global_opts.silent {
                println!("Successfully converted TEXT to DDS at {:?}", output_path);
            }
        }
        Command::ConvertTextureMapToTga(cmd) => {
            let tex = read_texture(&cmd, cli.global_opts.silent)?;

            let output_path = get_output_path(&cli.global_opts.output_path, &cmd.input_path, "tga");

            let tga = convert::create_tga(&tex)
                .context("Failed to create TGA from the texture map")?;
            fs::write(&output_path, tga)
                .with_context(|| format!("Failed to write TGA file to {:?}", output_path))?;

            if !cli.global_opts.silent {
                println!("Successfully converted TEXT to TGA at {:?}", output_path);
            }
        }
        Command::ConvertTgaToTextureMap(cmd) => {
            let tga_data = fs::read(&cmd.input_path)
                .with_context(|| format!("Failed to read TGA file at {:?}", cmd.input_path))?;
            let mut cursor = Cursor::new(tga_data);

            let tex = TextureMapBuilder::from_tga(&mut cursor)
                .context("Failed to create TextureMapBuilder from TGA data")?
                .with_mipblock1(!cmd.no_texd)
                .build(cmd.game_version)
                .context("Failed to build TextureMap from TGA data")?;

            let text_path = get_output_path(&cli.global_opts.output_path, &cmd.input_path, "TEXT");

            fs::write(&text_path, tex.pack_to_vec().context("Failed to pack TEXT data")?)
                .with_context(|| format!("Failed to write TEXT file to {:?}", text_path))?;

            if !cli.global_opts.silent {
                println!("Successfully converted TGA to TEXT at {:?}", text_path);
            }

            if tex.has_mipblock1() {
                let texd_path = text_path.with_extension("TEXD");
                fs::write(
                    &texd_path,
                    tex.mipblock1()
                        .context("Failed to retrieve TEXD data from TextureMap")?
                        .pack_to_vec(cmd.game_version)
                        .context("Failed to pack TEXD data")?,
                )
                    .with_context(|| format!("Failed to write TEXD file to {:?}", texd_path))?;

                if !cli.global_opts.silent {
                    println!("Successfully wrote TEXD at {:?}", texd_path);
                }
            }
        }
        Command::PortTextureMap(cmd) => {
            let tex = read_texture_port(&cmd)?;

            let output_path = get_output_path(&cli.global_opts.output_path, &cmd.input_path, "TEXT");

            let builder = TextureMapBuilder::from_texture_map(&tex)
                .context("Failed to create TextureMapBuilder from existing TextureMap")?
                .with_mipblock1(!cmd.no_texd);

            let ported_tex = builder.build(cmd.to_version)
                .context("Failed to build ported TextureMap")?;

            fs::write(&output_path, ported_tex.pack_to_vec().context("Failed to pack TEXT data")?)
                .with_context(|| format!("Failed to write ported TEXT file to {:?}", output_path))?;

            if !cli.global_opts.silent {
                println!("Successfully ported TEXT from {:?} to {:?} at {:?}", cmd.from_version, cmd.to_version, output_path);
            }

            if ported_tex.has_mipblock1() {
                let texd_path = output_path.with_extension("TEXD");
                fs::write(
                    &texd_path,
                    ported_tex.mipblock1()
                        .context("Failed to retrieve TEXD data from ported TextureMap")?
                        .pack_to_vec(cmd.to_version)
                        .context("Failed to pack TEXD data")?,
                )
                    .with_context(|| format!("Failed to write ported TEXD file to {:?}", texd_path))?;

                if !cli.global_opts.silent {
                    println!("Successfully wrote ported TEXD at {:?}", texd_path);
                }
            }
        }
    }

    Ok(())
}

/// Reads and parses a TextureMap from a TEXT file, applying TEXD data if provided.
/// This function is used for ConvertTextureMap and ConvertTgaToTextureMap commands.
fn read_texture(cmd: &ConvertTextureMap, silent: bool) -> Result<TextureMap> {
    let input_data = fs::read(&cmd.input_path)
        .with_context(|| format!("Failed to read input TEXT file at {:?}", cmd.input_path))?;

    let mut tex = TextureMap::from_memory(&input_data, cmd.game_version)
        .context("Failed to parse the TEXT file into TextureMap")?;

    if let Some(texd_path) = &cmd.texd_path {
        let texd_data = fs::read(texd_path)
            .with_context(|| format!("Failed to read TEXD file at {:?}", texd_path))?;
        tex.set_mipblock1(MipblockData::from_memory(&texd_data, cmd.game_version).context("Failed to apply TEXD data to TextureMap")?);

    }

    if !silent {
        println!("Successfully read the texture file:");
        println!("{:<15}: {:?}", "type", tex.texture_type());
        if let Some(interpret) = tex.interpret_as() {
            println!("{:<15}: {:?}", "interpret", interpret);
        }
        println!("{:<15}: {}x{}", "size", tex.width(), tex.height());
        println!("{:<15}: {}", "mip amount", tex.num_mip_levels());
        println!("{:<15}: {}", "has atlas", tex.has_atlas());
    }

    Ok(tex)
}

/// Reads and parses a TextureMap from a TEXT file specifically for porting.
/// This function ensures that the source version matches the provided from_version.
fn read_texture_port(cmd: &PortTextureMap) -> Result<TextureMap> {
    let input_data = fs::read(&cmd.input_path)
        .with_context(|| format!("Failed to read input TEXT file at {:?}", cmd.input_path))?;

    let tex = TextureMap::from_memory(&input_data, cmd.from_version)
        .context("Failed to parse the TEXT file into TextureMap")?;

    if let Some(texd_path) = &cmd.texd_path {
        let texd_data = fs::read(texd_path)
            .with_context(|| format!("Failed to read TEXD file at {:?}", texd_path))?;
        let mut tex_with_texd = tex.clone();
        tex_with_texd.set_mipblock1(MipblockData::from_memory(&texd_data, cmd.from_version).context("Failed to apply TEXD data to TextureMap")?);
        Ok(tex_with_texd)
    } else {
        Ok(tex)
    }
}

/// Determines the output path based on the provided global output path or defaults to the input path with a new extension.
fn get_output_path(global_output: &Option<String>, input_path: &str, new_ext: &str) -> PathBuf {
    global_output
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(input_path).with_extension(new_ext))
}