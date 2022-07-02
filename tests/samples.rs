use ck3save::{
    models::{Gamestate, HeaderBorrowed, HeaderOwned},
    Ck3File, Encoding, EnvTokens,
};
mod utils;

#[test]
fn test_ck3_text_header() {
    let data = include_bytes!("fixtures/header.txt");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::Text);
    let header = file.parse_metadata().unwrap();
    let header: HeaderOwned = header.deserializer().build(&EnvTokens).unwrap();
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_text_header_borrowed() {
    let data = include_bytes!("fixtures/header.txt");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::Text);
    let header = file.parse_metadata().unwrap();
    let header: HeaderBorrowed = header.deserializer().build(&EnvTokens).unwrap();
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_text_save() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("Jarl_Ivar_of_the_Isles_867_01_01.ck3");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::TextZip);
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink).unwrap();
    let game: Gamestate = parsed_file.deserializer().build(&EnvTokens).unwrap();
    assert_eq!(game.meta_data.version, String::from("1.0.2"));
    Ok(())
}
