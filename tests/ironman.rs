use ck3save::{
    models::{Gamestate, HeaderBorrowed, HeaderOwned},
    BasicTokenResolver, Ck3File, Encoding, FailedResolveStrategy,
};
use jomini::binary::TokenResolver;
use std::{io::Cursor, sync::LazyLock};

mod utils;

static TOKENS: LazyLock<BasicTokenResolver> = LazyLock::new(|| {
    let file_data = std::fs::read("assets/ck3.txt").unwrap_or_default();
    BasicTokenResolver::from_text_lines(file_data.as_slice()).unwrap()
});

macro_rules! skip_if_no_tokens {
    () => {
        if TOKENS.is_empty() {
            return;
        }
    };
}

pub fn request_zip<S: AsRef<str>>(input: S) -> Vec<u8> {
    use std::io::{Cursor, Read};
    let data = utils::request(input);
    let reader = Cursor::new(&data[..]);
    let mut zip = zip::ZipArchive::new(reader).unwrap();
    let mut zip_file = zip.by_index(0).unwrap();
    let mut buffer = Vec::with_capacity(0);
    zip_file.read_to_end(&mut buffer).unwrap();
    buffer
}

#[test]
fn test_ck3_binary_header() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::Binary);
    let meta = file.meta();
    let mut zip_sink = Vec::new();
    let header = meta.parse(&mut zip_sink).unwrap();
    let header: HeaderOwned = header.deserializer(&*TOKENS).deserialize().unwrap();
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_binary_header_borrowed() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::Binary);
    let meta = file.meta();
    let mut zip_sink = Vec::new();
    let header = meta.parse(&mut zip_sink).unwrap();
    let header: HeaderBorrowed = header.deserializer(&*TOKENS).deserialize().unwrap();
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_binary_save() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }

    let data = utils::request("af_Munso_867_Ironman.ck3");
    let file = Ck3File::from_slice(&data[..])?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;
    let game: Gamestate = parsed_file.deserializer(&*TOKENS).deserialize()?;
    assert_eq!(game.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_ck3_binary_save_header_borrowed() {
    skip_if_no_tokens!();
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let meta = file.meta();
    let mut zip_sink = Vec::new();
    let header = meta.parse(&mut zip_sink).unwrap();
    let header: HeaderBorrowed = header.deserializer(&*TOKENS).deserialize().unwrap();
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    assert_eq!(header.meta_data.version, "1.0.2");
}

#[test]
fn test_ck3_binary_compressed_header() {
    skip_if_no_tokens!();
    let data = utils::request("von_konigsberg_867_01_01.ck3 ");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let meta = file.meta();
    let mut out = Cursor::new(Vec::new());
    meta.melter().melt(&mut out, &*TOKENS).unwrap();
    memchr::memmem::find(&out.get_ref(), b"meta_real_date=124.11.1").unwrap();
}

#[test]
fn test_ck3_binary_autosave() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = request_zip("autosave.zip");

    let file = Ck3File::from_slice(&data[..])?;
    assert_eq!(file.encoding(), Encoding::Binary);

    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;
    let game: Gamestate = parsed_file.deserializer(&*TOKENS).deserialize()?;
    assert_eq!(game.meta_data.version, String::from("1.0.2"));

    let meta = file.meta();
    let mut zip_sink = Vec::new();
    let header = meta.parse(&mut zip_sink).unwrap();
    let header: HeaderBorrowed = header.deserializer(&*TOKENS).deserialize()?;
    assert_eq!(header.meta_data.version, String::from("1.0.2"));

    let mut out = Cursor::new(Vec::new());
    file.melter().melt(&mut out, &*TOKENS)?;
    memchr::memmem::find(&out.get_ref(), b"gold=0.044").unwrap();
    memchr::memmem::find(&out.get_ref(), b"gold=4.647").unwrap();

    Ok(())
}

#[test]
fn test_ck3_binary_save_tokens() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("af_Munso_867_Ironman.ck3");
    let file = Ck3File::from_slice(&data[..])?;
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;
    let save: Gamestate = parsed_file.deserializer(&*TOKENS).deserialize()?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_roundtrip_header_melt() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    let header = file.meta();
    let mut out = Cursor::new(Vec::new());
    header.melter().melt(&mut out, &*TOKENS).unwrap();

    let file = Ck3File::from_slice(&out.get_ref()).unwrap();
    let meta = file.meta();
    let mut zip_sink = Vec::new();
    let header = meta.parse(&mut zip_sink).unwrap();
    let header: HeaderOwned = header.deserializer(&*TOKENS).deserialize().unwrap();

    assert_eq!(file.encoding(), Encoding::Text);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_header_melt() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    let meta = file.meta();
    let mut out = Cursor::new(Vec::new());
    meta.melter().melt(&mut out, &*TOKENS).unwrap();

    let melted = include_bytes!("fixtures/header.melted");
    assert_eq!(&melted[..], out.get_ref().as_slice());
}

#[test]
fn test_melt_no_crash() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/melt.crash1");
    assert!(Ck3File::from_slice(&data[..]).is_err());
}

#[test]
fn test_ck3_binary_save_patch_1_3() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("ck3-1.3-test.ck3");
    let file = Ck3File::from_slice(&data[..])?;
    let mut out = Cursor::new(Vec::new());
    file.melter().melt(&mut out, &*TOKENS)?;

    let file = Ck3File::from_slice(&out.get_ref())?;
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;

    let save: Gamestate = parsed_file
        .deserializer(&*TOKENS)
        .on_failed_resolve(FailedResolveStrategy::Error)
        .deserialize()?;
    assert_eq!(save.meta_data.version, String::from("1.3.0"));
    Ok(())
}

#[test]
fn test_ck3_1_0_3_old_cloud_and_local_tokens() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("ck3-1.0.3-local.ck3");

    let file = Ck3File::from_slice(&data[..])?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let mut out = Cursor::new(Vec::new());
    file.melter().melt(&mut out, &*TOKENS)?;

    let file = Ck3File::from_slice(&out.get_ref())?;
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;
    assert_eq!(file.encoding(), Encoding::Text);

    let save: Gamestate = parsed_file
        .deserializer(&*TOKENS)
        .on_failed_resolve(FailedResolveStrategy::Error)
        .deserialize()?;

    assert_eq!(save.meta_data.version, String::from("1.0.3"));
    Ok(())
}

#[test]
fn decode_and_melt_gold_correctly() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("ck3-1.3.1.ck3");
    let file = Ck3File::from_slice(&data)?;
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;
    let save: Gamestate = parsed_file
        .deserializer(&*TOKENS)
        .on_failed_resolve(FailedResolveStrategy::Error)
        .deserialize()?;

    assert_eq!(file.encoding(), Encoding::BinaryZip);

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

    let mut out = Cursor::new(Vec::new());
    file.melter().melt(&mut out, &*TOKENS)?;

    memchr::memmem::find(&out.get_ref(), b"gold=133.04397").unwrap();
    memchr::memmem::find(&out.get_ref(), b"vassal_power_value=200").unwrap();
    Ok(())
}

#[test]
fn parse_patch16() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("ck3-1.6.ck3");
    let file = Ck3File::from_slice(&data)?;
    let mut zip_sink = Vec::new();
    let parsed_file = file.parse(&mut zip_sink)?;
    let save: Gamestate = parsed_file
        .deserializer(&*TOKENS)
        .on_failed_resolve(FailedResolveStrategy::Error)
        .deserialize()?;

    assert_eq!(save.meta_data.version.as_str(), "1.6.0");
    Ok(())
}

#[test]
fn melt_patch14() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("ck3-1.4-normal.ck3");
    let expected = request_zip("ck3-1.4-normal_melted.zip");
    let file = Ck3File::from_slice(&data[..])?;
    let mut out = Cursor::new(Vec::new());
    file.melter().melt(&mut out, &*TOKENS)?;

    assert!(
        eq(&out.get_ref(), &expected),
        "patch 1.4 did not melt currently"
    );
    Ok(())
}

#[test]
fn melt_patch15() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::request("ck3-1.5-normal.ck3");
    let expected = request_zip("ck3-1.5-normal_melted.zip");
    let file = Ck3File::from_slice(&data[..])?;
    let mut out = Cursor::new(Vec::new());
    file.melter().melt(&mut out, &*TOKENS)?;

    assert!(
        eq(&out.get_ref(), &expected),
        "patch 1.5 did not melt currently"
    );
    Ok(())
}

fn eq(a: &[u8], b: &[u8]) -> bool {
    for (ai, bi) in a.iter().zip(b.iter()) {
        if ai != bi {
            return false;
        }
    }

    a.len() == b.len()
}
