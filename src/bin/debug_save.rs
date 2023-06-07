use std::env;

use ck3save::{models::Gamestate, Ck3File, EnvTokens};
use serde::Deserialize;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let data = std::fs::read(&args[1])?;
    let file = Ck3File::from_slice(&data)?;
    let mut zip_sink = Vec::new();
    let file = file.parse(&mut zip_sink)?;
    let deserializer = file.deserializer(&EnvTokens);
    let result: Result<Gamestate, _> = serde_path_to_error::deserialize(deserializer);
    match result {
        Ok(_) => panic!("expected a type error"),
        Err(err) => {
            let path = err.path().to_string();
            assert_eq!(path, "dependencies.serde.version");
        }
    }
    // let save: Gamestate = deserializer.deserialize()?;
    // println!("{:#?}", save.meta_data.version);
    // println!("{:#?}", save.played_character.character);
    // println!("{:#?}", save.living.get(&save.played_character.character));
    // let traits = save
    //     .living
    //     .get(&save.played_character.character)
    //     .unwrap()
    //     .traits
    //     .clone()
    //     .unwrap();
    // let trait_strings = traits
    //     .iter()
    //     .map(|t| save.traits_lookup[*t].clone())
    //     .collect::<Vec<String>>();
    // println!(
    //     "{:#?}",
    //     traits
    //         .iter()
    //         .map(|t| save.traits_lookup[*t].clone())
    //         .collect::<Vec<String>>()
    // );
    Ok(())
}
