use std::{fs};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::exit;
use binrw::BinRead;
use clap::{Args, Parser, Subcommand};
use tex_rs::texture_map::{TextureMap};
use tex_rs::WoaVersion;
use tex_rs::convert;

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
    /// Convert a TEXT file to a dds file
    ConvertTextureMapToDDS(ConvertTextureMap),

    /// Convert a TEXT file to a tga file
    ConvertTextureMapToTga(ConvertTextureMap),
}

#[derive(Debug, Args)]
struct GlobalOpts {

    /// The directory to output to, if there's any
    #[clap(short, long, global = true)]
    output_path: Option<String>

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

    /// Version of the game you want to convert from options: [HM2016, HM2, HM3]
    #[clap(short, long)]
    game_version: WoaVersion,

    /// Path to the .text file
    #[arg(short, long)]
    input_path: String,

    /// Enable this to generate only a .text file
    #[arg(long)]
    no_texd: bool,
}

fn main() {
    let cli = Cli::parse();
    // let cli = Cli{
    //     global_opts: GlobalOpts { output_path: None },
    //     command : Command::ConvertTextureMapToTga(ConvertTextureMap { game_version: WoaVersion::HM3, input_path: "/home/dafitius/Documents/Hitman modding/Tools/rpkgtools2.19/chunk0/TEXT/0057756D027C1DDB.TEXT".to_string(), texd_path: None })
    // };

    match cli.command {
        Command::ConvertTextureMapToDDS(cmd) => {

            let mut stream = Cursor::new(fs::read(Path::new(&cmd.input_path)).unwrap());
            let mut tex = TextureMap::read_le_args(&mut stream, (cmd.game_version, )).unwrap_or_else(|e| {
                println!("Failed to parse the file: {}", e);
                exit(1);
            });
            
            if let Some(texd_path) = cmd.texd_path {
                tex.set_mipblock1_data(&fs::read(Path::new(&texd_path)).unwrap(), cmd.game_version).unwrap_or_else(|e|{
                    println!("Failed to apply the texd file: {}", e);
                    exit(1);
                });
            }

            println!("Successfully read the texture file:");
            println!("{:<15}: {:?}", "type", tex.get_header().interpret_as);
            println!("{:<15}: {}x{}", "size", tex.width(), tex.height());
            println!("{:<15}: {}", "mip amount", tex.get_num_mip_levels());
            println!("{:<15}: {}", "has atlas" , tex.has_atlas());

            let output_path = match cli.global_opts.output_path {
                Some(ref p) => { PathBuf::from(p) },
                None => {Path::new(&cmd.input_path).with_extension("dds")}
            };

            let dds = convert::create_dds(&tex).unwrap();
            fs::write(output_path.as_path(), dds).expect("TODO: panic message");
        },
        Command::ConvertTextureMapToTga(cmd) => {

            let mut stream = Cursor::new(std::fs::read(Path::new(&cmd.input_path)).unwrap());
            let mut tex = TextureMap::read_le_args(&mut stream, (cmd.game_version, )).unwrap();

            if let Some(texd_path) = cmd.texd_path {
                tex.set_mipblock1_data(&fs::read(Path::new(&texd_path)).unwrap(), cmd.game_version).expect("TODO: panic message");
            }

            let output_path = match cli.global_opts.output_path {
                Some(ref p) => { PathBuf::from(p) },
                None => {Path::new(&cmd.input_path).with_extension("tga")}
            };

            let tga = convert::create_tga(&tex).unwrap();
            fs::write(output_path.as_path(), tga).unwrap()
        }
        // Add other subcommand branches here
    }
}