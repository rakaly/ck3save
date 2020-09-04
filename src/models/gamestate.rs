use super::MetadataOwned;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Gamestate {
    pub meta_data: MetadataOwned,
}
