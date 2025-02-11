use ck3save::{
    file::{Ck3FsFileKind, Ck3ParsedText},
    BasicTokenResolver, Ck3File,
};
use std::{env, error::Error, io::Read};

fn json_to_stdout(file: &Ck3ParsedText) {
    let stdout = std::io::stdout();
    let _ = file.reader().json().to_writer(stdout.lock());
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let file = std::fs::File::open(&args[1])?;
    let mut file = Ck3File::from_file(file)?;

    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    let resolver = BasicTokenResolver::from_text_lines(file_data.as_slice())?;

    let melt_options = ck3save::MeltOptions::new();
    match file.kind_mut() {
        Ck3FsFileKind::Text(x) => {
            let mut buf = Vec::new();
            x.read_to_end(&mut buf)?;
            let text = Ck3ParsedText::from_raw(&buf)?;
            json_to_stdout(&text);
        }
        Ck3FsFileKind::Binary(x) => {
            let mut buf = Vec::new();
            x.melt(melt_options, resolver, &mut buf)?;
            let text = Ck3ParsedText::from_slice(&buf)?;
            json_to_stdout(&text);
        }
        Ck3FsFileKind::Zip(x) => {
            let mut data = Vec::new();
            x.melt(melt_options, resolver, &mut data)?;
            let text = Ck3ParsedText::from_slice(&data)?;
            json_to_stdout(&text);
        }
    }

    Ok(())
}
