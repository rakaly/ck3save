#![no_main]
use ck3save::EnvTokens;
use libfuzzer_sys::fuzz_target;

fn run(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let file = ck3save::Ck3File::from_slice(&data)?;

    let meta = file.parse_metadata()?;
    let _meta: Result<ck3save::models::HeaderBorrowed, _> =
        meta.deserializer(&EnvTokens).deserialize();

    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;

    match parsed_file.kind() {
        ck3save::file::Ck3ParsedFileKind::Text(x) => {
            x.reader().json().to_writer(std::io::sink())?;
        }
        ck3save::file::Ck3ParsedFileKind::Binary(x) => {
            x.melter().melt(&EnvTokens)?;
        }
    }

    let _game: Result<ck3save::models::Gamestate, _> =
        parsed_file.deserializer(&EnvTokens).deserialize();

    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let _ = run(data);
});
