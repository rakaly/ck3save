use ck3save::{Ck3File, EnvTokens, FailedResolveStrategy};
use std::{env, io::BufWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1])?;
    let file = Ck3File::from_slice(&data)?;
    let stdout = std::io::stdout();
    let handle = stdout.lock();
    let buffer = BufWriter::new(handle);
    file.melter()
        .on_failed_resolve(FailedResolveStrategy::Error)
        .melt(buffer, &EnvTokens)?;
    Ok(())
}
