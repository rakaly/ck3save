#![cfg(ironman)]
use ck3save::{Ck3Extractor, Encoding, FailedResolveStrategy};
use std::io::{Cursor, Read};

mod utils;

#[test]
fn test_ck3_binary_header() {
    let data = include_bytes!("fixtures/header.bin");
    let (header, encoding) = Ck3Extractor::extract_header(&data[..]).unwrap();
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_binary_header_borrowed() {
    let data = include_bytes!("fixtures/header.bin");
    let (header, encoding) = Ck3Extractor::builder()
        .extract_header_borrowed(&data[..])
        .unwrap();
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(header.meta_data.version, "1.0.2");
}

#[test]
fn test_ck3_binary_save() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let reader = Cursor::new(&data[..]);
    let (save, encoding) = Ck3Extractor::extract_save(reader)?;
    assert_eq!(encoding, Encoding::BinaryZip);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_ck3_binary_save_header_borrowed() {
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let (header, encoding) = Ck3Extractor::builder()
        .extract_header_borrowed(&data[..])
        .unwrap();
    assert_eq!(encoding, Encoding::BinaryZip);
    assert_eq!(header.meta_data.version, "1.0.2");
}

#[test]
fn test_ck3_binary_autosave() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("autosave.ck3.zip");
    let reader = Cursor::new(&data[..]);
    let mut zip = zip::ZipArchive::new(reader).unwrap();
    let mut zip_file = zip.by_index(0).unwrap();
    let mut buffer = Vec::with_capacity(0);
    zip_file.read_to_end(&mut buffer).unwrap();

    let reader = Cursor::new(&buffer[..]);
    let (save, encoding) = Ck3Extractor::extract_save(reader)?;
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_ck3_binary_save_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let reader = Cursor::new(&data[..]);
    let (save, encoding) = Ck3Extractor::builder()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .extract_save(reader)?;
    assert_eq!(encoding, Encoding::BinaryZip);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_roundtrip_header_melt() {
    let data = include_bytes!("fixtures/header.bin");
    let out = ck3save::Melter::new().melt(&data[..]).unwrap();
    let (header, encoding) = Ck3Extractor::extract_header(&out).unwrap();
    assert_eq!(encoding, Encoding::TextZip);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_header_melt() {
    let data = include_bytes!("fixtures/header.bin");
    let melted = include_bytes!("fixtures/header.melted");
    let out = ck3save::Melter::new().melt(&data[..]).unwrap();
    assert_eq!(&melted[..], &out[..]);
}
