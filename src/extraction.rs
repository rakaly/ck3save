/*!
CK3 save files can be encoded in 3 different formats:

 - autosave
 - standard
 - ironman

Let's start with standard and ironman first. These two are similar in that there are three
sections:

 - a save id line
 - the header
 - a zip with the compressed gamestate

For standard saves, the header and the compressed gamestate are plaintext. Ironman saves use the
standard PDS binary format (not explained here).

What is interesting is that the gamestate contains the same header info. So one can bypass the
header and skip right to the zip file and there won't be any loss of data.

Now for autosave format:

 - a save id line
 - uncompressed gamestate in the binary format

These 3 formats pose an interesting challenge. If we only looked for the zip file signature (to
split the file to ensure our parser doesn't start interpretting zip data), we may end up scanning
100MB worth of data before realizing it's an autosave. This would be bad for performance. The
solution is to take advantage that zips orient themselves at the end of the file, so we assume
a zip until further notice.

In short, to know what the save file format:

- Attempt to parse as zip
- if not a zip, we know it's an autosave (uncompressed binary)
- else if the 3rd and 4th byte are `01 00` then we know it's ironman
- else it's a standard save
*/

use crate::{
    flavor::flavor_from_tape,
    models::{Gamestate, HeaderBorrowed, HeaderOwned},
    tokens::TokenLookup,
    Ck3Error, Ck3ErrorKind, FailedResolveStrategy,
};
use jomini::{BinaryDeserializer, BinaryTape, TextDeserializer, TokenResolver};
use serde::de::{Deserialize, DeserializeOwned};
use std::io::{Cursor, Read, Seek, SeekFrom};
use zip::{result::ZipError, ZipArchive};

/// Describes the format of the save before decoding
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoding {
    /// Save is encoded with the standard format:
    ///
    ///  - a save id line
    ///  - plaintext header
    ///  - zip with compressed plaintext gamestate
    TextZip,

    /// Save is encoded in the binary zip format
    ///
    ///  - a save id line
    ///  - binary header
    ///  - zip with compressed binary gamestate
    BinaryZip,

    /// Save is encoded in the binary format
    ///
    ///  - a save id line
    ///  - uncompressed binary gamestate
    Binary,
}

/// The memory allocation strategy for handling zip files
///
/// When the `mmap` feature is enabled, the extractor can use
/// an anonymous memory map
#[derive(Debug, Clone, Copy)]
pub enum Extraction {
    /// Extract the zip data into memory
    InMemory,

    /// Extract the zip data into a anonymous memory map
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
    /// and ignore unknown binary tokens
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

    /// Extract the header from the save.
    pub fn extract_header_owned(&self, data: &[u8]) -> Result<(HeaderOwned, Encoding), Ck3Error> {
        self.extract_header_as(data)
    }

    /// Extract the header from the save with zero copy deserialization.
    pub fn extract_header_borrowed<'a>(
        &self,
        data: &'a [u8],
    ) -> Result<(HeaderBorrowed<'a>, Encoding), Ck3Error> {
        self.extract_header_as(data)
    }

    /// Extract the header from the save as a custom type
    pub fn extract_header_with_tokens_as<'de, T, Q>(
        &self,
        data: &'de [u8],
        resolver: &'de Q,
    ) -> Result<(T, Encoding), Ck3Error>
    where
        T: Deserialize<'de>,
        Q: TokenResolver,
    {
        let data = skip_save_prefix(data);
        let mut cursor = Cursor::new(data);
        let offset = match detect_encoding(&mut cursor)? {
            BodyEncoding::Plain => data.len(),
            BodyEncoding::Zip(zip) => zip.offset() as usize,
        };

        let is_zipped = offset != data.len();
        let data = &data[..offset];
        if sniff_is_binary(data) {
            let encoding = if is_zipped {
                Encoding::BinaryZip
            } else {
                Encoding::Binary
            };
            let tape = BinaryTape::from_slice(data)?;
            let flavor = flavor_from_tape(&tape);
            let res = BinaryDeserializer::builder_flavor(flavor)
                .on_failed_resolve(self.on_failed_resolve)
                .from_tape(&tape, resolver)?;
            Ok((res, encoding))
        } else {
            // allow uncompressed text as TextZip even though the game doesn't produce said format
            let res = TextDeserializer::from_utf8_slice(data)?;
            Ok((res, Encoding::TextZip))
        }
    }

    /// Extract the header from the save as a custom type
    pub fn extract_header_as<'de, T>(&self, data: &'de [u8]) -> Result<(T, Encoding), Ck3Error>
    where
        T: Deserialize<'de>,
    {
        self.extract_header_with_tokens_as(data, &TokenLookup)
    }

    /// Extract all info from a save
    pub fn extract_save<R>(&self, reader: R) -> Result<(Gamestate, Encoding), Ck3Error>
    where
        R: Read + Seek,
    {
        self.extract_save_as(reader)
    }

    /// Extract all info from a save as a custom type
    pub fn extract_save_with_tokens_as<T, R, Q>(
        &self,
        mut reader: R,
        resolver: &Q,
    ) -> Result<(T, Encoding), Ck3Error>
    where
        R: Read + Seek,
        T: DeserializeOwned,
        Q: TokenResolver,
    {
        let mut buffer = Vec::new();
        match detect_encoding(&mut reader)? {
            BodyEncoding::Plain => {
                // Ensure we are at the start of the stream
                reader.seek(SeekFrom::Start(0))?;

                // So we can get the length
                let len = reader.seek(SeekFrom::End(0))?;
                reader.seek(SeekFrom::Start(0))?;
                buffer.reserve(len as usize);
                reader.read_to_end(&mut buffer)?;

                let data = skip_save_prefix(&buffer);
                let tape = BinaryTape::from_slice(data)?;
                let flavor = flavor_from_tape(&tape);
                let res = BinaryDeserializer::builder_flavor(flavor)
                    .on_failed_resolve(self.on_failed_resolve)
                    .from_tape(&tape, resolver)?;
                Ok((res, Encoding::Binary))
            }
            BodyEncoding::Zip(mut zip) => match self.extraction {
                Extraction::InMemory => melt_in_memory(
                    &mut buffer,
                    "gamestate",
                    &mut zip,
                    self.on_failed_resolve,
                    resolver,
                ),
                #[cfg(feature = "mmap")]
                Extraction::MmapTemporaries => {
                    melt_with_temporary("gamestate", &mut zip, self.on_failed_resolve, resolver)
                }
            },
        }
    }

    /// Extract all info from a save as a custom type
    pub fn extract_save_as<T, R>(&self, reader: R) -> Result<(T, Encoding), Ck3Error>
    where
        R: Read + Seek,
        T: DeserializeOwned,
    {
        self.extract_save_with_tokens_as(reader, &TokenLookup)
    }
}

/// Logic container for extracting data from an CK3 save
#[derive(Debug, Clone)]
pub struct Ck3Extractor {}

impl Ck3Extractor {
    /// Create a customized container
    pub fn builder() -> Ck3ExtractorBuilder {
        Ck3ExtractorBuilder::new()
    }

    /// Extract just the header from the save
    pub fn extract_header(data: &[u8]) -> Result<(HeaderOwned, Encoding), Ck3Error> {
        Self::builder().extract_header_owned(data)
    }

    /// Extract all info from a save
    pub fn extract_save<R>(reader: R) -> Result<(Gamestate, Encoding), Ck3Error>
    where
        R: Read + Seek,
    {
        Self::builder().extract_save(reader)
    }
}

fn melt_in_memory<T, R, Q>(
    buffer: &mut Vec<u8>,
    name: &'static str,
    zip: &mut zip::ZipArchive<R>,
    on_failed_resolve: FailedResolveStrategy,
    resolver: &Q,
) -> Result<(T, Encoding), Ck3Error>
where
    R: Read + Seek,
    T: DeserializeOwned,
    Q: TokenResolver,
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
        .read_to_end(buffer)
        .map_err(|e| Ck3ErrorKind::ZipExtraction(name, e))?;

    if sniff_is_binary(buffer) {
        let tape = BinaryTape::from_slice(buffer)?;
        let flavor = flavor_from_tape(&tape);
        let res = BinaryDeserializer::builder_flavor(flavor)
            .on_failed_resolve(on_failed_resolve)
            .from_tape(&tape, resolver)
            .map_err(|e| Ck3ErrorKind::Deserialize {
                part: Some(name.to_string()),
                err: e,
            })?;

        Ok((res, Encoding::BinaryZip))
    } else {
        let res = TextDeserializer::from_utf8_slice(buffer)?;
        Ok((res, Encoding::TextZip))
    }
}

#[cfg(feature = "mmap")]
fn melt_with_temporary<T, R, Q>(
    name: &'static str,
    zip: &mut zip::ZipArchive<R>,
    on_failed_resolve: FailedResolveStrategy,
    resolver: &Q,
) -> Result<(T, Encoding), Ck3Error>
where
    R: Read + Seek,
    T: DeserializeOwned,
    Q: TokenResolver,
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

    if sniff_is_binary(buffer) {
        let tape = BinaryTape::from_slice(buffer)?;
        let flavor = flavor_from_tape(&tape);
        let res = BinaryDeserializer::builder_flavor(flavor)
            .on_failed_resolve(on_failed_resolve)
            .from_tape(&tape, resolver)
            .map_err(|e| Ck3ErrorKind::Deserialize {
                part: Some(name.to_string()),
                err: e,
            })?;
        Ok((res, Encoding::BinaryZip))
    } else {
        let res = TextDeserializer::from_utf8_slice(buffer)?;
        Ok((res, Encoding::TextZip))
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

/// We guess from the initial data (after the save id line) that the save is
/// binary if the 3rd and 4th byte are the binary equals token, which should
/// not occur in a plaintext save
fn sniff_is_binary(data: &[u8]) -> bool {
    data.get(2..4).map_or(false, |x| x == [0x01, 0x00])
}

pub(crate) enum BodyEncoding<'a, R>
where
    R: Read + Seek,
{
    Zip(ZipArchive<&'a mut R>),
    Plain,
}

pub(crate) fn detect_encoding<R>(reader: &mut R) -> Result<BodyEncoding<R>, Ck3Error>
where
    R: Read + Seek,
{
    let zip_attempt = zip::ZipArchive::new(reader);

    match zip_attempt {
        Ok(x) => Ok(BodyEncoding::Zip(x)),
        Err(ZipError::InvalidArchive(_)) => Ok(BodyEncoding::Plain),
        Err(e) => Err(Ck3Error::new(Ck3ErrorKind::ZipCentralDirectory(e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_save_prefix() {
        let data = b"abc\n123";
        let result = skip_save_prefix(&data[..]);
        assert_eq!(result, b"123");
    }
}
