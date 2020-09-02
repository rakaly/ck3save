use std::fmt;
use std::io::Error as IoError;
use zip::result::ZipError;

/// An Ck3 Error
#[derive(Debug)]
pub struct Ck3Error(Box<Ck3ErrorKind>);

impl Ck3Error {
    pub(crate) fn new(kind: Ck3ErrorKind) -> Ck3Error {
        Ck3Error(Box::new(kind))
    }

    /// Return the specific type of error
    pub fn kind(&self) -> &Ck3ErrorKind {
        &self.0
    }
}

/// Specific type of error
#[derive(Debug)]
pub enum Ck3ErrorKind {
    ZipCentralDirectory(ZipError),
    ZipMissingEntry(&'static str, ZipError),
    ZipExtraction(&'static str, IoError),
    ZipSize(&'static str),
    IoErr(IoError),
    UnknownHeader,
    UnknownToken {
        token_id: u16,
    },
    Deserialize {
        part: Option<String>,
        err: jomini::Error,
    },
}

impl fmt::Display for Ck3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            Ck3ErrorKind::ZipCentralDirectory(_) => {
                write!(f, "unable to read zip central directory")
            }
            Ck3ErrorKind::ZipMissingEntry(s, _) => write!(f, "unable to locate {} in zip", s),
            Ck3ErrorKind::ZipExtraction(s, _) => write!(f, "unable to extract {} in zip", s),
            Ck3ErrorKind::ZipSize(s) => write!(f, "{} in zip is too large", s),
            Ck3ErrorKind::IoErr(_) => write!(f, "io error"),
            Ck3ErrorKind::UnknownHeader => write!(f, "unknown header encountered in zip"),
            Ck3ErrorKind::UnknownToken { token_id } => {
                write!(f, "unknown binary token encountered (id: {})", token_id)
            }
            Ck3ErrorKind::Deserialize { ref part, ref err } => match part {
                Some(p) => write!(f, "error deserializing: {}: {}", p, err),
                None => err.fmt(f),
            },
        }
    }
}

impl std::error::Error for Ck3Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            Ck3ErrorKind::ZipCentralDirectory(e) => Some(e),
            Ck3ErrorKind::ZipMissingEntry(_, e) => Some(e),
            Ck3ErrorKind::ZipExtraction(_, e) => Some(e),
            Ck3ErrorKind::IoErr(e) => Some(e),
            Ck3ErrorKind::Deserialize { ref err, .. } => Some(err),
            _ => None,
        }
    }
}

impl From<jomini::Error> for Ck3Error {
    fn from(err: jomini::Error) -> Self {
        Ck3Error::new(Ck3ErrorKind::Deserialize { part: None, err })
    }
}

impl From<IoError> for Ck3Error {
    fn from(err: IoError) -> Self {
        Ck3Error::new(Ck3ErrorKind::IoErr(err))
    }
}

impl From<Ck3ErrorKind> for Ck3Error {
    fn from(err: Ck3ErrorKind) -> Self {
        Ck3Error::new(err)
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
