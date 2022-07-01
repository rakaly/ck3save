use ck3save::{
    file::{Ck3ParsedFile, Ck3ParsedFileKind, Ck3Text},
    Ck3File, EnvTokens,
};
use std::env;

fn json_to_stdout(file: &Ck3Text) {
    let _ = file.reader().json().to_writer(std::io::stdout());
}

fn parsed_file_to_json(file: &Ck3ParsedFile) -> Result<(), Box<dyn std::error::Error>> {
    // if the save is binary, melt it, as the JSON API only works with text
    match file.kind() {
        Ck3ParsedFileKind::Text(text) => json_to_stdout(text),
        Ck3ParsedFileKind::Binary(binary) => {
            let melted = binary.melter().verbatim(true).melt(&EnvTokens)?;
            json_to_stdout(&Ck3Text::from_slice(melted.data())?);
        }
    };

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1]).unwrap();

    let file = Ck3File::from_slice(&data)?;
    let mut zip_sink = Vec::new();
    let file = file.parse(&mut zip_sink)?;
    parsed_file_to_json(&file)?;

    Ok(())
}
