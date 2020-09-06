use crate::{
    tokens::TokenLookup, zip_index, Ck3Date, Ck3Error, Ck3ErrorKind, Ck3Flavor, Extraction,
    FailedResolveStrategy, HEADER_LEN_UPPER_BOUND,
};
use jomini::{BinaryTape, BinaryToken, TokenResolver};
use std::io::{Cursor, Read, Write};

/// Convert a binary gamestate to plaintext
///
/// Accepted inputs:
///
/// - autosave save
/// - ironman save
/// - binary data
#[derive(Debug)]
pub struct Melter {
    on_failed_resolve: FailedResolveStrategy,
    extraction: Extraction,
}

impl Default for Melter {
    fn default() -> Self {
        Melter {
            extraction: Extraction::InMemory,
            on_failed_resolve: FailedResolveStrategy::Ignore,
        }
    }
}

impl Melter {
    /// Create a customized version to melt binary data
    pub fn new() -> Self {
        Melter::default()
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

    fn convert(&self, input: &[u8], writer: &mut Vec<u8>) -> Result<(), Ck3Error> {
        let tape = BinaryTape::parser_flavor(Ck3Flavor).parse_slice(input)?;
        let mut depth = 0;
        let mut in_objects: Vec<i32> = Vec::new();
        let mut in_object = 1;
        let mut token_idx = 0;
        let tokens = tape.tokens();

        while let Some(token) = tokens.get(token_idx) {
            let mut did_change = false;
            if in_object == 1 {
                let depth = match token {
                    BinaryToken::End(_) => depth - 1,
                    _ => depth,
                };

                for _ in 0..depth {
                    writer.push(b' ');
                }
            }

            match token {
                BinaryToken::Object(_) => {
                    did_change = true;
                    writer.extend_from_slice(b"{\r\n");
                    depth += 1;
                    in_objects.push(in_object);
                    in_object = 1;
                }
                BinaryToken::Array(_) => {
                    did_change = true;
                    writer.push(b'{');
                    depth += 1;
                    in_objects.push(in_object);
                    in_object = 0;
                }
                BinaryToken::End(_) => {
                    writer.push(b'}');
                    let obj = in_objects.pop();

                    // The binary parser should already ensure that this will be something, but this is
                    // just a sanity check
                    debug_assert!(obj.is_some());
                    in_object = obj.unwrap_or(1);
                    depth -= 1;
                }
                BinaryToken::Bool(x) => match x {
                    true => writer.extend_from_slice(b"yes"),
                    false => writer.extend_from_slice(b"no"),
                },
                BinaryToken::U32(x) => writer.extend_from_slice(format!("{}", x).as_bytes()),
                BinaryToken::U64(x) => writer.extend_from_slice(format!("{}", x).as_bytes()),
                BinaryToken::I32(x) => {
                    if let Some(date) = Ck3Date::from_i32(*x) {
                        writer.extend_from_slice(date.ck3_fmt().as_bytes());
                    } else {
                        writer.extend_from_slice(format!("{}", x).as_bytes());
                    }
                }
                BinaryToken::Text(x) => {
                    let data = x.view_data();
                    let end_idx = match data.last() {
                        Some(x) if *x == b'\n' => data.len() - 1,
                        Some(_x) => data.len(),
                        None => data.len(),
                    };
                    if in_object == 1 {
                        writer.extend_from_slice(&data[..end_idx]);
                    } else {
                        writer.push(b'"');
                        writer.extend_from_slice(&data[..end_idx]);
                        writer.push(b'"');
                    }
                }
                BinaryToken::F32_1(x) => write!(writer, "{}", x).map_err(Ck3ErrorKind::IoErr)?,
                BinaryToken::F32_2(x) => write!(writer, "{}", x).map_err(Ck3ErrorKind::IoErr)?,
                BinaryToken::Token(x) => match TokenLookup.resolve(*x) {
                    Some(id) if id == "is_ironman" && in_object == 1 => {
                        let skip = tokens
                            .get(token_idx + 1)
                            .map(|next_token| match next_token {
                                BinaryToken::Object(end) => end + 1,
                                BinaryToken::Array(end) => end + 1,
                                _ => token_idx + 2,
                            })
                            .unwrap_or(token_idx + 1);

                        token_idx = skip;
                        continue;
                    }
                    Some(id) => writer.extend_from_slice(&id.as_bytes()),
                    None => match self.on_failed_resolve {
                        FailedResolveStrategy::Error => {
                            return Err(Ck3ErrorKind::UnknownToken { token_id: *x }.into());
                        }
                        FailedResolveStrategy::Ignore if in_object == 1 => {
                            let skip = tokens
                                .get(token_idx + 1)
                                .map(|next_token| match next_token {
                                    BinaryToken::Object(end) => end + 1,
                                    BinaryToken::Array(end) => end + 1,
                                    _ => token_idx + 2,
                                })
                                .unwrap_or(token_idx + 1);

                            token_idx = skip;
                            continue;
                        }
                        _ => {
                            let unknown = format!("__unknown_0x{:x}", x);
                            writer.extend_from_slice(unknown.as_bytes());
                        }
                    },
                },
                BinaryToken::Rgb(color) => {
                    writer.extend_from_slice(b"rgb {");
                    writer.extend_from_slice(format!("{} ", color.r).as_bytes());
                    writer.extend_from_slice(format!("{} ", color.g).as_bytes());
                    writer.extend_from_slice(format!("{}", color.b).as_bytes());
                    writer.push(b'}');
                }
            }

            if !did_change && in_object == 1 {
                writer.push(b'=');
                in_object = 2;
            } else if in_object == 2 {
                in_object = 1;
                writer.push(b'\r');
                writer.push(b'\n');
            } else if in_object != 1 {
                writer.push(b' ');
            }

            token_idx += 1;
        }

        Ok(())
    }

    pub fn melt(&self, data: &[u8]) -> Result<Vec<u8>, Ck3Error> {
        let mut result = Vec::with_capacity(data.len());

        // if there is a save id line in the data, we should preserve it
        let has_save_id = data.get(0..3).map_or(false, |x| x == b"SAV");
        let data = if has_save_id {
            let split_ind = data.iter().position(|&x| x == b'\n').unwrap_or(0);
            let at = std::cmp::max(split_ind, 0);
            let (header, rest) = data.split_at(at + 1);
            result.extend_from_slice(header);
            rest
        } else {
            data
        };

        let cutoff = std::cmp::min(data.len(), HEADER_LEN_UPPER_BOUND);
        let zip_searchspace = &data[..cutoff];
        if zip_index(zip_searchspace).is_some() {
            let zip_reader = Cursor::new(data);
            let mut zip =
                zip::ZipArchive::new(zip_reader).map_err(Ck3ErrorKind::ZipCentralDirectory)?;
            let size = zip
                .by_name("gamestate")
                .map_err(|e| Ck3ErrorKind::ZipMissingEntry("gamestate", e))
                .map(|x| x.size())?;
            result.reserve(size as usize);

            let mut zip_file = zip
                .by_name("gamestate")
                .map_err(|e| Ck3ErrorKind::ZipMissingEntry("gamestate", e))?;

            match self.extraction {
                Extraction::InMemory => {
                    let mut inflated_data: Vec<u8> = Vec::with_capacity(size as usize);
                    zip_file
                        .read_to_end(&mut inflated_data)
                        .map_err(|e| Ck3ErrorKind::ZipExtraction("gamestate", e))?;
                    self.convert(&inflated_data, &mut result)?
                }

                #[cfg(feature = "mmap")]
                Extraction::MmapTemporaries => {
                    let mut mmap = memmap::MmapMut::map_anon(zip_file.size() as usize)?;
                    std::io::copy(&mut zip_file, &mut mmap.as_mut())
                        .map_err(|e| Ck3ErrorKind::ZipExtraction("gamestate", e))?;
                    self.convert(&mmap[..], &mut result)?
                }
            }
        } else {
            self.convert(data, &mut result)?
        };

        Ok(result)
    }
}
