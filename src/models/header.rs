use serde::Deserialize;
use std::borrow::Cow;

#[derive(Debug, Deserialize)]
pub struct HeaderOwned {
    pub meta_data: MetadataOwned,
}

#[derive(Debug, Deserialize)]
pub struct HeaderBorrowed<'a> {
    #[serde(borrow)]
    pub meta_data: MetadataBorrowed<'a>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataOwned {
    pub version: String,
    pub meta_player_name: String
}

#[derive(Debug, Deserialize)]
pub struct MetadataBorrowed<'a> {
    #[serde(borrow)]
    pub version: Cow<'a, str>,
}
