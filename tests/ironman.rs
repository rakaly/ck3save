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
    let data = utils::request("autosave.zip");
    let reader = Cursor::new(&data[..]);
    let mut zip = zip::ZipArchive::new(reader).unwrap();
    let mut zip_file = zip.by_index(0).unwrap();
    let mut buffer = Vec::with_capacity(0);
    zip_file.read_to_end(&mut buffer).unwrap();

    let reader = Cursor::new(&buffer[..]);
    let (save, encoding) = Ck3Extractor::extract_save(reader)?;
    assert_eq!(encoding, Encoding::Binary);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));

    let (save, encoding) = Ck3Extractor::extract_header(&buffer[..])?;
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
    let (out, _tokens) = ck3save::Melter::new().melt(&data[..]).unwrap();
    let (header, encoding) = Ck3Extractor::extract_header(&out).unwrap();
    assert_eq!(encoding, Encoding::TextZip);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_header_melt() {
    let data = include_bytes!("fixtures/header.bin");
    let melted = include_bytes!("fixtures/header.melted");
    let (out, _tokens) = ck3save::Melter::new().melt(&data[..]).unwrap();
    assert_eq!(&melted[..], &out[..]);
}

#[test]
fn test_melt_no_crash() {
    let data = include_bytes!("fixtures/melt.crash1");
    let _ = ck3save::Melter::new().melt(&data[..]);
}

#[test]
fn test_ck3_binary_save_patch_1_3() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("ck3-1.3-test.ck3");
    let (_out, _tokens) = ck3save::Melter::new()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .melt(&data)?;
    let reader = Cursor::new(&data[..]);
    let (save, _encoding) = Ck3Extractor::extract_save(reader)?;
    assert_eq!(save.meta_data.version, String::from("1.3.0"));
    Ok(())
}

#[test]
fn test_ck3_1_0_3_old_cloud_and_local_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("ck3-1.0.3-local.ck3");
    let (_out, _tokens) = ck3save::Melter::new()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .melt(&data)?;

    let reader = Cursor::new(&data[..]);
    let (save, encoding) = Ck3Extractor::builder()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .extract_save(reader)?;
    assert_eq!(encoding, Encoding::BinaryZip);
    assert_eq!(save.meta_data.version, String::from("1.0.3"));
    Ok(())
}

#[test]
fn decode_and_melt_gold_correctly() -> Result<(), Box<dyn std::error::Error>> {
    let data = utils::request("ck3-1.3.1.ck3");
    let reader = Cursor::new(&data[..]);
    let (save, encoding) = Ck3Extractor::builder()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .extract_save(reader)?;
    assert_eq!(encoding, Encoding::BinaryZip);
    let character = save.living.get(&16322).unwrap();
    assert_eq!(
        character.alive_data.as_ref().and_then(|x| x.health),
        Some(4.728)
    );
    assert_eq!(
        character.alive_data.as_ref().and_then(|x| x.income),
        Some(11.087)
    );
    assert_eq!(
        character.alive_data.as_ref().and_then(|x| x.gold),
        Some(133.04397)
    );

    let (out, _tokens) = ck3save::Melter::new()
        .with_on_failed_resolve(FailedResolveStrategy::Error)
        .melt(&data)?;

    twoway::find_bytes(&out, b"gold=133.04397").unwrap();

    Ok(())
}
