use std::env;

use ck3save::{models::Gamestate, BasicTokenResolver, Ck3File};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let file = std::fs::File::open(&args[1])?;
    let mut file = Ck3File::from_file(file)?;
    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    let resolver = BasicTokenResolver::from_text_lines(file_data.as_slice())?;
    let save = Gamestate::from_file(&mut file, &resolver)?;
    print!("{:#?}", save.meta_data.version);
    Ok(())
}
