use crate::{
    models::{Ck3Save, Header},
    tokens::TokenLookup,
    Ck3Error, Ck3ErrorKind, FailedResolveStrategy,
};
use jomini::{BinaryDeserializer, TextDeserializer};
use serde::de::DeserializeOwned;
use std::io::{Cursor, Read, Seek};

/// Describes the format of the save before decoding
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoding {
    /// Save is regular save
    Text,

    /// Save is an binary save
    Binary,
}

/// The memory allocation strategy for handling zip files
///
/// When the `mmap` feature is enabled, the
#[derive(Debug, Clone, Copy)]
pub enum Extraction {
    /// Extract the zip data into memory
    InMemory,

    /// Extract the zip data into a temporary file that is memmapped
    #[cfg(feature = "mmap")]
    MmapTemporaries,
}

/// Customize how a save is extracted
#[derive(Debug, Clone)]
pub struct Ck3ExtractorBuilder {
    extraction: Extraction,
    on_failed_resolve: FailedResolveStrategy,
}

impl Default for Ck3ExtractorBuilder {
    fn default() -> Self {
        Ck3ExtractorBuilder::new()
    }
}

impl Ck3ExtractorBuilder {
    /// Create a new extractor with default values: extract zips into memory
    // and ignore unknown binary tokens
    pub fn new() -> Self {
        Ck3ExtractorBuilder {
            extraction: Extraction::InMemory,
            on_failed_resolve: FailedResolveStrategy::Ignore,
        }
    }

    /// Set the memory allocation extraction behavior for when a zip is encountered
    pub fn with_extraction(mut self, extraction: Extraction) -> Self {
        self.extraction = extraction;
        self
    }

    /// Set the behavior for when an unresolved binary token is encountered
    pub fn with_on_failed_resolve(mut self, strategy: FailedResolveStrategy) -> Self {
        self.on_failed_resolve = strategy;
        self
    }

    /// Extract just the header from the save.
    pub fn extract_header(&self, data: &[u8]) -> Result<(Header, Encoding), Ck3Error> {
        let data = skip_save_prefix(&data);
        let (header, _gamestate) = split_on_zip(data);
        let is_ironman = header.get(2..4).map_or(false, |x| x == &[0x01, 0x00]);
        if is_ironman {
            let res = BinaryDeserializer::builder()
                .on_failed_resolve(self.on_failed_resolve)
                .from_slice(header, &TokenLookup)?;
            Ok((res, Encoding::Binary))
        } else {
            let res = TextDeserializer::from_slice(header)?;
            Ok((res, Encoding::Text))
        }
    }

    /// Extract all info from a save
    pub fn extract_save(&self, data: &[u8]) -> Result<(Ck3Save, Encoding), Ck3Error> {
        let data = skip_save_prefix(&data);
        let (header, _gamestate) = split_on_zip(data);
        let is_ironman = header.get(2..4).map_or(false, |x| x == &[0x01, 0x00]);
        let header = if is_ironman {
            BinaryDeserializer::builder()
                .on_failed_resolve(self.on_failed_resolve)
                .from_slice(header, &TokenLookup)
        } else {
            TextDeserializer::from_slice(header)
        }.map_err(|err| Ck3Error::new(Ck3ErrorKind::Deserialize { part: Some(String::from("header")), err }))?;

        let encoding = if is_ironman {
            Encoding::Binary
        } else {
            Encoding::Text
        };

        let mut buffer = Vec::new();
        let mut reader = Cursor::new(data);
        let mut zip =
            zip::ZipArchive::new(&mut reader).map_err(Ck3ErrorKind::ZipCentralDirectory)?;
        let gamestate = match self.extraction {
            Extraction::InMemory => melt_in_memory(
                &mut buffer,
                "gamestate",
                &mut zip,
                self.on_failed_resolve,
                encoding,
            )?,
            #[cfg(feature = "mmap")]
            Extraction::MmapTemporaries => {
                melt_with_temporary("gamestate", &mut zip, self.on_failed_resolve, encoding)?
            }
        };

        let result = Ck3Save {
            header,
            gamestate,
        };
        Ok((result, encoding))
    }
}

/// Logic container for extracting data from an Ck3 save
#[derive(Debug, Clone)]
pub struct Ck3Extractor {}

impl Ck3Extractor {
    /// Create a customized container
    pub fn builder() -> Ck3ExtractorBuilder {
        Ck3ExtractorBuilder::new()
    }

    /// Extract just the metadata from the save. This can be efficiently done if
    /// a file is zip encoded.
    pub fn extract_header(data: &[u8]) -> Result<(Header, Encoding), Ck3Error> {
        Self::builder().extract_header(data)
    }

    /// Extract all info from a save
    pub fn extract_save(data: &[u8]) -> Result<(Ck3Save, Encoding), Ck3Error> {
        Self::builder().extract_save(data)
    }
}

fn melt_in_memory<T, R>(
    mut buffer: &mut Vec<u8>,
    name: &'static str,
    zip: &mut zip::ZipArchive<R>,
    on_failed_resolve: FailedResolveStrategy,
    encoding: Encoding,
) -> Result<T, Ck3Error>
where
    R: Read + Seek,
    T: DeserializeOwned,
{
    buffer.clear();
    let mut zip_file = zip
        .by_name(name)
        .map_err(|e| Ck3ErrorKind::ZipMissingEntry(name, e))?;

    // protect against excessively large uncompressed data
    if zip_file.size() > 1024 * 1024 * 200 {
        return Err(Ck3ErrorKind::ZipSize(name).into());
    }

    buffer.reserve(zip_file.size() as usize);
    zip_file
        .read_to_end(&mut buffer)
        .map_err(|e| Ck3ErrorKind::ZipExtraction(name, e))?;

    if encoding == Encoding::Binary {
        let res = BinaryDeserializer::builder()
            .on_failed_resolve(on_failed_resolve)
            .from_slice(&buffer, &TokenLookup)
            .map_err(|e| Ck3ErrorKind::Deserialize {
                part: Some(name.to_string()),
                err: e,
            })?;
        Ok(res)
    } else {
        let res = TextDeserializer::from_slice(&buffer)?;
        Ok(res)
    }
}

#[cfg(feature = "mmap")]
fn melt_with_temporary<T, R>(
    name: &'static str,
    zip: &mut zip::ZipArchive<R>,
    on_failed_resolve: FailedResolveStrategy,
) -> Result<T, Ck3Error>
where
    R: Read + Seek,
    T: DeserializeOwned,
{
    let mut zip_file = zip
        .by_name(name)
        .map_err(|e| Ck3ErrorKind::ZipMissingEntry(name, e))?;

    // protect against excessively large uncompressed data
    if zip_file.size() > 1024 * 1024 * 200 {
        return Err(Ck3ErrorKind::ZipSize(name).into());
    }

    let mut mmap = memmap::MmapMut::map_anon(zip_file.size() as usize)?;
    std::io::copy(&mut zip_file, &mut mmap.as_mut())
        .map_err(|e| Ck3ErrorKind::ZipExtraction(name, e))?;
    let buffer = &mmap[..];

    if encoding == Encoding::Binary {
        let res = BinaryDeserializer::builder()
            .on_failed_resolve(on_failed_resolve)
            .from_slice(&buffer, &TokenLookup)
            .map_err(|e| Ck3ErrorKind::Deserialize {
                part: Some(name.to_string()),
                err: e,
            })?;
        Ok(res)
    } else {
        let res = TextDeserializer::from_slice(&buffer)?;
        Ok(res)
    }
}

/// Throwaway the first line
fn skip_save_prefix(data: &[u8]) -> &[u8] {
    let id_line_idx = data
        .iter()
        .position(|&x| x == b'\n')
        .map(|x| x + 1)
        .unwrap_or(0);
    &data[id_line_idx..]
}

/// The save embeds a zip after the header. This function finds the zip magic code
fn split_on_zip(data: &[u8]) -> (&[u8], &[u8]) {
    if let Some(idx) = twoway::find_bytes(data, &[0x50, 0x4b, 0x03, 0x04]) {
        data.split_at(idx)
    } else {
        data.split_at(data.len())
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_skip_save_prefix() {
        let data = b"abc\n123";
        let result = skip_save_prefix(&data[..]);
        assert_eq!(result, b"123");
    }
}