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

    #[error("unable to inflate zip entry: {source}")]
    ZipInflation {
        #[source]
        source: std::io::Error,
    },

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_of_error_test() {
        assert_eq!(std::mem::size_of::<Ck3Error>(), 8);
    }
}
