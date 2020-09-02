use ck3save::{Ck3Extractor, Encoding};

mod utils;

#[test]
fn test_ck3_text_header() {
    let data = include_bytes!("fixtures/header.txt");
    let (header, encoding) = Ck3Extractor::extract_header(&data[..]).unwrap();
    assert_eq!(encoding, Encoding::Text);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_text_save() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("Jarl_Ivar_of_the_Isles_867_01_01.ck3");
    let (save, encoding) = Ck3Extractor::extract_save(&data[..])?;
    assert_eq!(encoding, Encoding::Text);
    assert_eq!(save.gamestate.meta_data.version, String::from("1.0.2"));
    Ok(())
}
