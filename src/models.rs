use serde::Deserialize;

#[derive(Debug)]
pub struct Ck3Save {
    pub header: Header,
    pub gamestate: Gamestate,
}

#[derive(Debug, Deserialize)]
pub struct Header {
    pub meta_data: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Gamestate {
    pub meta_data: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub version: String,
}
