use std::env;

use ck3save::{models::Gamestate, Ck3File, EnvTokens};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1])?;
    let file = Ck3File::from_slice(&data)?;
    let mut zip_sink = Vec::new();
    let file = file.parse(&mut zip_sink)?;
    let save: Gamestate = file.deserializer().build(&EnvTokens)?;
    print!("{:#?}", save.meta_data.version);
    Ok(())
}
