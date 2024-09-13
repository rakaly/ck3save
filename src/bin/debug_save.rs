use std::env;

use ck3save::{models::Gamestate, BasicTokenResolver, Ck3File};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1])?;
    let file = Ck3File::from_slice(&data)?;
    let mut zip_sink = Vec::new();
    let file = file.parse(&mut zip_sink)?;
    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    let resolver = BasicTokenResolver::from_text_lines(file_data.as_slice())?;
    let save: Gamestate = file.deserializer(&resolver).deserialize()?;
    print!("{:#?}", save.meta_data.version);
    Ok(())
}
