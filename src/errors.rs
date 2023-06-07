use crate::deflate::ZipInflationError;
use std::fmt;
use zip::result::ZipError;

/// A Ck3 Error
#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct Ck3Error(#[from] Box<Ck3ErrorKind>);

impl Ck3Error {
    pub(crate) fn new(kind: Ck3ErrorKind) -> Ck3Error {
        Ck3Error(Box::new(kind))
    }

    /// Return the specific type of error
    pub fn kind(&self) -> &Ck3ErrorKind {
        &self.0
    }
}

impl From<Ck3ErrorKind> for Ck3Error {
    fn from(err: Ck3ErrorKind) -> Self {
        Ck3Error::new(err)
    }
}

/// Specific type of error
#[derive(thiserror::Error, Debug)]
pub enum Ck3ErrorKind {
    #[error("unable to parse as zip: {0}")]
    ZipArchive(#[from] ZipError),

    #[error("missing gamestate entry in zip")]
    ZipMissingEntry,

    #[error("unable to inflate zip entry: {msg}")]
    ZipBadData { msg: String },

    #[error("early eof, only able to write {written} bytes")]
    ZipEarlyEof { written: usize },

    #[error("unable to parse due to: {0}")]
    Parse(#[source] jomini::Error),

    #[error("unable to deserialize due to: {0}")]
    Deserialize(#[source] jomini::Error),

    #[error("error while writing output: {0}")]
    Writer(#[source] jomini::Error),

    #[error("unknown binary token encountered: {token_id:#x}")]
    UnknownToken { token_id: u16 },

    #[error("invalid header")]
    InvalidHeader,

    #[error("expected the binary integer: {0} to be parsed as a date")]
    InvalidDate(i32),

    #[error("unable to deserialize due to: {msg}. This shouldn't occur as this is a deserializer wrapper")]
    DeserializeImpl { msg: String },
}

impl From<ZipInflationError> for Ck3ErrorKind {
    fn from(x: ZipInflationError) -> Self {
        match x {
            ZipInflationError::BadData { msg } => Ck3ErrorKind::ZipBadData { msg },
            ZipInflationError::EarlyEof { written } => Ck3ErrorKind::ZipEarlyEof { written },
        }
    }
}

impl serde::de::Error for Ck3Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Ck3Error::new(Ck3ErrorKind::DeserializeImpl {
            msg: msg.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_of_error_test() {
        assert_eq!(std::mem::size_of::<Ck3Error>(), 8);
    }
}
