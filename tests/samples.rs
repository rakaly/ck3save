use ck3save::{file::Ck3SliceFileKind, models::Header, Ck3File, Encoding};
use std::collections::HashMap;
mod utils;

#[test]
fn test_ck3_text_header() {
    let data = include_bytes!("fixtures/header.txt");
    let file = Ck3File::from_slice(&data[..]).unwrap();
    assert_eq!(file.encoding(), Encoding::Text);

    let header: Header = match file.kind() {
        Ck3SliceFileKind::Text(text) => text.deserializer().deserialize().unwrap(),
        Ck3SliceFileKind::Binary(_) => panic!("impossible"),
        Ck3SliceFileKind::Zip(zip) => zip
            .meta()
            .unwrap()
            .deserializer(&HashMap::<u16, &str>::new())
            .deserialize()
            .unwrap(),
    };

    assert_eq!(header.meta_data.version, String::from("1.0.2"));
}

#[test]
fn test_ck3_text_save() -> Result<(), Box<dyn std::error::Error>> {
    let file = utils::request_file("Jarl_Ivar_of_the_Isles_867_01_01.ck3");
    let mut file = Ck3File::from_file(file).unwrap();
    assert_eq!(file.encoding(), Encoding::TextZip);
    let game = file.parse_save(HashMap::<u16, &str>::new()).unwrap();
    assert_eq!(game.meta_data.version, String::from("1.0.2"));
    Ok(())
}
