use ck3save::{BasicTokenResolver, Ck3File, Ck3Melt, JominiFileKind, SaveDataKind};
use jomini::TextTape;
use std::{env, error::Error, io::Read};

fn json_to_stdout(data: &[u8]) -> Result<(), Box<dyn Error>> {
    let tape = TextTape::from_slice(data)?;
    let stdout = std::io::stdout();
    let _ = tape.utf8_reader().json().to_writer(stdout.lock());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let file = std::fs::File::open(&args[1])?;
    let mut file = Ck3File::from_file(file)?;

    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    let resolver = BasicTokenResolver::from_text_lines(file_data.as_slice())?;

    let melt_options = ck3save::MeltOptions::new();
    let mut buf = Vec::new();
    match file.kind_mut() {
        JominiFileKind::Uncompressed(SaveDataKind::Text(x)) => {
            x.body().cursor().read_to_end(&mut buf)?;
            json_to_stdout(&buf)?;
        }
        JominiFileKind::Uncompressed(SaveDataKind::Binary(x)) => {
            x.melt(melt_options, resolver, &mut buf)?;
            json_to_stdout(&buf)?;
        }
        JominiFileKind::Zip(x) => {
            x.melt(melt_options, resolver, &mut buf)?;
            json_to_stdout(&buf)?;
        }
    };

    Ok(())
}
