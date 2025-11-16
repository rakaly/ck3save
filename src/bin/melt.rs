use ck3save::{BasicTokenResolver, Ck3File, Ck3Melt, FailedResolveStrategy, MeltOptions};
use std::{env, io::BufWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let file = std::fs::File::open(&args[1])?;
    let mut file = Ck3File::from_file(file)?;
    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    let resolver = BasicTokenResolver::from_text_lines(file_data.as_slice())?;
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let buffer = BufWriter::new(handle);
    file.melt(
        MeltOptions::new().on_failed_resolve(FailedResolveStrategy::Error),
        resolver,
        buffer,
    )?;
    Ok(())
}
