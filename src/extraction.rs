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

One knows the header is done when the zip file signature is encountered:

```ignore
50 4B 03 04
```

What is interesting is that the gamestate contains the same header info. So one can bypass the
header and skip right to the zip file and there won't be any loss of data.

Now for autosave format:

 - a save id line
 - uncompressed gamestate in the binary format

These 3 formats pose an interesting challenge. If we only looked for the zip file signature (to
split the file to ensure our parser doesn't start interpretting zip data), we may end up scanning
100MB worth of data before realizing it's an autosave. This would be bad for performance. The
solution is to take advantage that headers are less than 64KB long. If the zip signature appears
there, we chop off whatever zip data was included and parse the remaining.

In short, to know what the save file format:

- Take the first 64KB
- if there is no zip signature, we know it's an autosave (uncompressed binary)
- else if the 3rd and 4th byte are `01 00` then we know it's ironman
- else it's a standard save
*/

use crate::{
    models::{Gamestate, HeaderBorrowed, HeaderOwned},
    tokens::TokenLookup,
    Ck3Error, Ck3ErrorKind, FailedResolveStrategy,
};
use jomini::{BinaryDeserializer, BinaryFlavor, TextDeserializer};
use serde::de::{Deserialize, DeserializeOwned};
use std::io::{Read, Seek};

// The amount of data that we will scan up to looking for a zip signature
pub(crate) const HEADER_LEN_UPPER_BOUND: usize = 0x10000;

pub(crate) struct Ck3Flavor;
impl BinaryFlavor for Ck3Flavor {
    fn visit_f32_1(&self, data: &[u8]) -> f32 {
        unsafe { std::ptr::read_unaligned(data.as_ptr() as *const u8 as *const f32) }
    }

    fn visit_f32_2(&self, data: &[u8]) -> f32 {
        let val = unsafe { std::ptr::read_unaligned(data.as_ptr() as *const u8 as *const i32) };
        (val as f32) / 1000.0
    }
}

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
    pub fn extract_header_as<'de, T>(&self, data: &'de [u8]) -> Result<(T, Encoding), Ck3Error>
    where
        T: Deserialize<'de>,
    {
        let data = skip_save_prefix(&data);
        let data = &data[..std::cmp::min(data.len(), HEADER_LEN_UPPER_BOUND)];
        let (header, rest) = split_on_zip(data);
        if sniff_is_binary(header) {
            let res = BinaryDeserializer::builder_flavor(Ck3Flavor)
                .on_failed_resolve(self.on_failed_resolve)
                .from_slice(header, &TokenLookup)?;

            if rest.is_empty() {
                Ok((res, Encoding::Binary))
            } else {
                Ok((res, Encoding::BinaryZip))
            }
        } else {
            let res = TextDeserializer::from_slice(header)?;
            Ok((res, Encoding::TextZip))
        }
    }

    /// Extract all info from a save
    pub fn extract_save<R>(&self, reader: R) -> Result<(Gamestate, Encoding), Ck3Error>
    where
        R: Read + Seek,
    {
        self.extract_save_as(reader)
    }

    /// Extract all info from a save as a custom type
    pub fn extract_save_as<T, R>(&self, mut reader: R) -> Result<(T, Encoding), Ck3Error>
    where
        R: Read + Seek,
        T: DeserializeOwned,
    {
        // First we need to determine if we are in an autosave or a header + zip save.
        // We determine this by examining the first 64KB and if the zip magic header
        // occurs then we know it is a zip file.
        let mut buffer = vec![0; HEADER_LEN_UPPER_BOUND];
        read_upto(&mut reader, &mut buffer)?;

        if zip_index(&buffer).is_some() {
            let mut zip =
                zip::ZipArchive::new(&mut reader).map_err(Ck3ErrorKind::ZipCentralDirectory)?;
            match self.extraction {
                Extraction::InMemory => {
                    melt_in_memory(&mut buffer, "gamestate", &mut zip, self.on_failed_resolve)
                }
                #[cfg(feature = "mmap")]
                Extraction::MmapTemporaries => {
                    melt_with_temporary("gamestate", &mut zip, self.on_failed_resolve)
                }
            }
        } else {
            reader.read_to_end(&mut buffer)?;

            let data = skip_save_prefix(&buffer);
            let res = BinaryDeserializer::builder_flavor(Ck3Flavor)
                .on_failed_resolve(self.on_failed_resolve)
                .from_slice(data, &TokenLookup)?;
            Ok((res, Encoding::Binary))
        }
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

fn melt_in_memory<T, R>(
    mut buffer: &mut Vec<u8>,
    name: &'static str,
    zip: &mut zip::ZipArchive<R>,
    on_failed_resolve: FailedResolveStrategy,
) -> Result<(T, Encoding), Ck3Error>
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

    if sniff_is_binary(&buffer) {
        let res = BinaryDeserializer::builder_flavor(Ck3Flavor)
            .on_failed_resolve(on_failed_resolve)
            .from_slice(&buffer, &TokenLookup)
            .map_err(|e| Ck3ErrorKind::Deserialize {
                part: Some(name.to_string()),
                err: e,
            })?;
        Ok((res, Encoding::BinaryZip))
    } else {
        let res = TextDeserializer::from_slice(&buffer)?;
        Ok((res, Encoding::TextZip))
    }
}

#[cfg(feature = "mmap")]
fn melt_with_temporary<T, R>(
    name: &'static str,
    zip: &mut zip::ZipArchive<R>,
    on_failed_resolve: FailedResolveStrategy,
) -> Result<(T, Encoding), Ck3Error>
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

    if sniff_is_binary(buffer) {
        let res = BinaryDeserializer::builder_flavor(Ck3Flavor)
            .on_failed_resolve(on_failed_resolve)
            .from_slice(&buffer, &TokenLookup)
            .map_err(|e| Ck3ErrorKind::Deserialize {
                part: Some(name.to_string()),
                err: e,
            })?;
        Ok((res, Encoding::BinaryZip))
    } else {
        let res = TextDeserializer::from_slice(&buffer)?;
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

/// Returns the index in the data where the zip occurs
pub(crate) fn zip_index(data: &[u8]) -> Option<usize> {
    twoway::find_bytes(data, &[0x50, 0x4b, 0x03, 0x04])
}

/// The save embeds a zip after the header. This function finds the zip magic code
/// and splits the data into two so they can be parsed separately.
fn split_on_zip(data: &[u8]) -> (&[u8], &[u8]) {
    if let Some(idx) = zip_index(data) {
        data.split_at(idx)
    } else {
        data.split_at(data.len())
    }
}

// Read until either the reader is out of data or the buffer is filled.
// This is essentially Read::read_exact without the validation at the end
fn read_upto<R>(reader: &mut R, mut buf: &mut [u8]) -> Result<(), std::io::Error>
where
    R: std::io::Read,
{
    while !buf.is_empty() {
        match reader.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                let tmp = buf;
                buf = &mut tmp[n..];
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
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
