use ck3save::{BasicTokenResolver, Ck3File, FailedResolveStrategy};
use std::{env, io::BufWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1])?;
    let file = Ck3File::from_slice(&data)?;
    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    let resolver = BasicTokenResolver::from_text_lines(file_data.as_slice())?;
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let buffer = BufWriter::new(handle);
    file.melter()
        .on_failed_resolve(FailedResolveStrategy::Error)
        .melt(buffer, &resolver)?;
    Ok(())
}
