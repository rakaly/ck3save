use ck3save::{
    file::{Ck3FsFileKind, Ck3SliceFileKind},
    models::{Gamestate, Header},
    BasicTokenResolver, Ck3File, Encoding, MeltOptions,
};
use jomini::binary::TokenResolver;
use std::{
    io::{Cursor, Read},
    sync::LazyLock,
};

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

#[test]
fn test_ck3_binary_header() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::Binary);

    let header: Header = match file.kind() {
        Ck3SliceFileKind::Text(_) => panic!("impossible"),
        Ck3SliceFileKind::Binary(binary) => {
            binary.clone().deserializer(&*TOKENS).deserialize().unwrap()
        }
        Ck3SliceFileKind::Zip(zip) => zip
            .meta()
            .unwrap()
            .deserializer(&*TOKENS)
            .deserialize()
            .unwrap(),
    };

    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_binary_save() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }

    let file = utils::request_file("af_Munso_867_Ironman.ck3");
    let mut file = Ck3File::from_file(file)?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let game = file.parse_save(&*TOKENS)?;
    assert_eq!(game.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_ck3_binary_compressed_header() {
    skip_if_no_tokens!();
    let file = utils::request_file("von_konigsberg_867_01_01.ck3");
    let file = Ck3File::from_file(file).unwrap();
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let Ck3FsFileKind::Zip(zip) = file.kind() else {
        panic!("unexpected type");
    };

    let mut meta = zip.meta().unwrap();
    let mut out = Cursor::new(Vec::new());
    meta.melt(MeltOptions::new(), &*TOKENS, &mut out).unwrap();
    memchr::memmem::find(out.get_ref(), b"meta_real_date=124.11.1").unwrap();
}

#[test]
fn test_ck3_binary_autosave() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let data = utils::inflate(utils::request_file("autosave.zip"));

    let file = Ck3File::from_slice(&data[..])?;
    assert_eq!(file.encoding(), Encoding::Binary);

    let game: Gamestate = file.parse_save(&*TOKENS)?;
    assert_eq!(game.meta_data.version, String::from("1.0.2"));

    let Ck3SliceFileKind::Binary(ref binary) = file.kind() else {
        panic!("unexpected type");
    };

    let header: Header = binary.clone().deserializer(&*TOKENS).deserialize()?;
    assert_eq!(header.meta_data.version, String::from("1.0.2"));

    let mut out = Cursor::new(Vec::new());
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;
    memchr::memmem::find(out.get_ref(), b"gold=0.044").unwrap();
    memchr::memmem::find(out.get_ref(), b"gold=4.647").unwrap();

    Ok(())
}

#[test]
fn test_ck3_binary_save_tokens() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let file = utils::request_file("af_Munso_867_Ironman.ck3");
    let mut file = Ck3File::from_file(file)?;
    let save: Gamestate = file.parse_save(&*TOKENS)?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    assert_eq!(save.meta_data.version, String::from("1.0.2"));
    Ok(())
}

#[test]
fn test_roundtrip_header_melt() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    let Ck3SliceFileKind::Binary(ref binary) = file.kind() else {
        panic!("unexpected type");
    };
    let mut out = Cursor::new(Vec::new());
    binary
        .clone()
        .melt(MeltOptions::new(), &*TOKENS, &mut out)
        .unwrap();

    let file = Ck3File::from_slice(out.get_ref()).unwrap();
    let Ck3SliceFileKind::Text(ref text) = file.kind() else {
        panic!("unexpected type");
    };
    let header: Header = text.deserializer().deserialize().unwrap();

    assert_eq!(file.encoding(), Encoding::Text);
    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_header_melt() {
    skip_if_no_tokens!();
    let data = include_bytes!("fixtures/header.bin");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    let Ck3SliceFileKind::Binary(ref binary) = file.kind() else {
        panic!("unexpected type");
    };
    let mut out = Cursor::new(Vec::new());
    binary
        .clone()
        .melt(MeltOptions::new(), &*TOKENS, &mut out)
        .unwrap();

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
    let file = utils::request_file("ck3-1.3-test.ck3");
    let mut file = Ck3File::from_file(file)?;
    let mut out = Cursor::new(Vec::new());
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;

    let file = Ck3File::from_slice(out.get_ref())?;
    let save: Gamestate = file.parse_save(&*TOKENS)?;
    assert_eq!(save.meta_data.version, String::from("1.3.0"));
    Ok(())
}

#[test]
fn test_ck3_1_0_3_old_cloud_and_local_tokens() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let file = utils::request_file("ck3-1.0.3-local.ck3");
    let mut file = Ck3File::from_file(file)?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let mut out = Cursor::new(Vec::new());
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;

    let file = Ck3File::from_slice(out.get_ref())?;
    assert_eq!(file.encoding(), Encoding::Text);
    let save: Gamestate = file.parse_save(&*TOKENS)?;

    assert_eq!(save.meta_data.version, String::from("1.0.3"));
    Ok(())
}

#[test]
fn decode_and_melt_gold_correctly() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let file = utils::request_file("ck3-1.3.1.ck3");
    let mut file = Ck3File::from_file(file)?;
    let save = file.parse_save(&*TOKENS)?;

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
        character
            .alive_data
            .as_ref()
            .and_then(|x| x.gold.as_ref())
            .map(|x| x.value()),
        Some(133.04397)
    );

    let mut out = Cursor::new(Vec::new());
    let file = utils::request_file("ck3-1.3.1.ck3");
    let mut file = Ck3File::from_file(file)?;
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;

    memchr::memmem::find(out.get_ref(), b"gold=133.04397").unwrap();
    memchr::memmem::find(out.get_ref(), b"vassal_power_value=200").unwrap();
    Ok(())
}

#[test]
fn parse_patch16() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }

    let file = utils::request_file("ck3-1.6.ck3");
    let mut file = Ck3File::from_file(file)?;
    assert_eq!(file.encoding(), Encoding::BinaryZip);
    let save = file.parse_save(&*TOKENS)?;
    assert_eq!(save.meta_data.version.as_str(), "1.6.0");
    Ok(())
}

#[test]
fn melt_patch14() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let file = utils::request_file("ck3-1.4-normal.ck3");
    let expected = utils::inflate(utils::request_file("ck3-1.4-normal_melted.zip"));
    let mut file = Ck3File::from_file(file)?;
    let mut out = Cursor::new(Vec::new());
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;

    assert_eq!(
        out.get_ref().as_slice(),
        expected.as_slice(),
        "patch 1.4 did not melt correctly"
    );
    Ok(())
}

#[test]
fn melt_patch15() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let file = utils::request_file("ck3-1.5-normal.ck3");
    let expected = utils::inflate(utils::request_file("ck3-1.5-normal_melted.zip"));
    let mut file = Ck3File::from_file(file)?;
    let mut out = Cursor::new(Vec::new());
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;

    assert_eq!(
        out.get_ref().as_slice(),
        expected.as_slice(),
        "patch 1.5 did not melt correctly"
    );
    Ok(())
}

#[test]
fn melt_patch15_slice() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let mut file = utils::request_file("ck3-1.5-normal.ck3");
    let mut content = Vec::new();
    file.read_to_end(&mut content)?;
    let expected = utils::inflate(utils::request_file("ck3-1.5-normal_melted.zip"));
    let file = Ck3File::from_slice(&content)?;
    let mut out = Cursor::new(Vec::new());
    file.melt(MeltOptions::new(), &*TOKENS, &mut out)?;

    assert_eq!(
        out.get_ref().as_slice(),
        expected.as_slice(),
        "patch 1.5 did not melt correctly"
    );
    Ok(())
}

#[test]
fn parse_patch_1_16_slice() -> Result<(), Box<dyn std::error::Error>> {
    if TOKENS.is_empty() {
        return Ok(());
    }
    let mut file = utils::request_file("patch_1_16.ck3");
    let mut content = Vec::new();
    file.read_to_end(&mut content)?;
    let file = Ck3File::from_slice(&content)?;
    let result = file.parse_save(&*TOKENS)?;
    assert_eq!(result.meta_data.version, String::from("1.16.2.3"));
    Ok(())
}
