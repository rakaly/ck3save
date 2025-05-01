use serde::Deserialize;
use crate::Ck3Date;

#[derive(Debug, Deserialize)]
pub struct Header {
    pub meta_data: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub version: String,
    pub meta_date: Ck3Date,
}
