use glacier_base::encryption::xtea::XteaConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ini = glacier_ini::IniFileSystem::from_path("./Retail/thumbs.dat", XteaConfig::Knt)?;
    println!("{:?}", ini);
    Ok(())
}
