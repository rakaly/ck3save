use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Header {
    pub meta_data: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub version: String,
}
