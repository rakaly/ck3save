use ck3save::{Ck3Extractor, Encoding};
use std::io::Cursor;

mod utils;

#[test]
fn test_ck3_text_header() {
    let data = include_bytes!("fixtures/header.txt");
    let (header, encoding) = Ck3Extractor::extract_header(&data[..]).unwrap();
    assert_eq!(encoding, Encoding::TextZip);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_text_header_borrowed() {
    let data = include_bytes!("fixtures/header.txt");
    let (header, encoding) = Ck3Extractor::builder()
        .extract_header_borrowed(&data[..])
        .unwrap();
    assert_eq!(encoding, Encoding::TextZip);
    assert_eq!(header.meta_data.version, "1.0.2");
}

#[test]
fn test_ck3_text_save() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("Jarl_Ivar_of_the_Isles_867_01_01.ck3");
    let reader = Cursor::new(&data[..]);
    let (save, encoding) = Ck3Extractor::extract_save(reader)?;
    assert_eq!(encoding, Encoding::TextZip);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));
    Ok(())
}
