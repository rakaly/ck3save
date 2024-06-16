use ck3save::{file::Ck3Text, Ck3File, EnvTokens};
use std::{env, io::Cursor};

fn json_to_stdout(file: &Ck3Text) {
    let _ = file.reader().json().to_writer(std::io::stdout());
}

fn parsed_file_to_json(file: &Ck3File) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = Cursor::new(Vec::new());
    file.melter().verbatim(true).melt(&mut out, &EnvTokens)?;
    json_to_stdout(&Ck3Text::from_slice(out.get_ref())?);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1]).unwrap();

    let file = Ck3File::from_slice(&data)?;
    parsed_file_to_json(&file)?;

    Ok(())
}
