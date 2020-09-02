#![cfg(ironman)]
use ck3save::{Ck3Extractor, Encoding, FailedResolveStrategy};

mod utils;

#[test]
fn test_ck3_binary_header() {
    let data = include_bytes!("fixtures/header.bin");
    let (header, encoding) = Ck3Extractor::extract_header(&data[..]).unwrap();
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_binary_save() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let (save, encoding) = Ck3Extractor::extract_save(&data[..])?;
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(save.gamestate.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_ck3_binary_save_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let (save, encoding) = Ck3Extractor::builder()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .extract_save(&data[..])?;
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(save.gamestate.meta_data.version, String::from("1.0.2"));
    Ok(())
}
