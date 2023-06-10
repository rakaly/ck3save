use std::collections::HashMap;


use serde::{Deserialize, Deserializer, Serialize};

use crate::{Ck3Date};
use crate::flavor::reencode_float;
use crate::maybe_object::MaybeObject;

use super::MetadataOwned;

#[derive(Debug, Deserialize)]
pub struct Gamestate {
    pub meta_data: MetadataOwned,
    pub living: HashMap<u64, LivingCharacter>,
    pub provinces: HashMap<u64, Province>,
    pub traits_lookup: Vec<String>,
    pub dynasties: Dynasties,
    pub religion: Religions,
    pub played_character: PlayedCharacter,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Dynasties {
    pub dynasty_house: HashMap<u64, MaybeObject<DynastyHouse>>,
    pub dynasties: HashMap<u64, MaybeObject<Dynasty>>,
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayedCharacter {
    pub name: String,
    pub character: u64
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DynastyHouse {
    pub name: Option<String>,
    // pub dynasty: Option<String>
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Dynasty {
    pub key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Province {
    pub holding: Holding,
    pub fort_level: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Holding {
    pub r#type: Option<String>,
    pub buildings: Vec<Building>,
    pub levy: Option<u64>,
    pub garrison: Option<u64>,
    pub income: Option<f32>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Building {
    pub r#type: Option<String>
}


#[derive(Debug, Deserialize, Serialize)]
pub struct LivingCharacter {
    pub alive_data: Option<AliveData>,
    pub first_name: String,
    pub dynasty_house: Option<u64>,
    pub birth: Option<Ck3Date>,
    #[serde(default = "default_false")]
    pub female: bool,
    pub traits: Option<Vec<usize>>,
    pub skill: Vec<u64>,
    pub faith: Option<u64>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AliveData {
    #[serde(default, deserialize_with = "deserialize_eu4_float")]
    pub gold: Option<f64>,
    pub health: Option<f32>,
    pub income: Option<f32>,
    pub fertility: Option<f32>,
    pub faith: Option<u64>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Religions {
    pub religions: HashMap<u64, Religion>,
    pub faiths: HashMap<u64, Faith>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Religion {
    pub tag: String,
    pub family: String
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Faith {
    pub tag: String,
    pub religion: u64
}

pub(crate) fn deserialize_eu4_float<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let val: Option<f64> = Option::deserialize(deserializer)?;
    val.map(reencode_float).map(Ok).transpose()
}

pub (crate) fn default_false() -> bool {
    false
}
