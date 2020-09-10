use ck3save::Ck3Extractor;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1])?;
    let (save, _encoding) = Ck3Extractor::extract_header(&data[..])?;
    print!("{:#?}", save.meta_data.version);
    Ok(())
}
