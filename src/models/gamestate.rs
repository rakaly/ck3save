use super::MetadataOwned;
use crate::util::reencode_float;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Gamestate {
    pub meta_data: MetadataOwned,
    pub living: HashMap<u64, LivingCharacter>,
}

#[derive(Debug, Deserialize)]
pub struct LivingCharacter {
    pub alive_data: Option<AliveData>,
}

#[derive(Debug, Deserialize)]
pub struct AliveData {
    #[serde(default, deserialize_with = "deserialize_eu4_float")]
    pub gold: Option<f32>,
    pub health: Option<f32>,
    pub income: Option<f32>,
}

pub(crate) fn deserialize_eu4_float<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let val: Option<f32> = Option::deserialize(deserializer)?;
    val.map(reencode_float).map(Ok).transpose()
}
