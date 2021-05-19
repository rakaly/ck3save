use crate::util::reencode_float;
use crate::{
    detect_encoding, tokens::TokenLookup, BodyEncoding, Ck3Date, Ck3Error, Ck3ErrorKind,
    Extraction, FailedResolveStrategy,
};
use jomini::{BinaryTape, BinaryToken, TokenResolver};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
};

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
    rewrite: bool,
}

impl Default for Melter {
    fn default() -> Self {
        Melter {
            extraction: Extraction::InMemory,
            on_failed_resolve: FailedResolveStrategy::Ignore,
            rewrite: true,
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

    /// Set if the melter should rewrite properties to better match the plaintext format
    ///
    /// Setting to false will preserve binary fields and values even if they
    /// don't make any sense in the plaintext output.
    pub fn with_rewrite(mut self, rewrite: bool) -> Self {
        self.rewrite = rewrite;
        self
    }

    fn convert(
        &self,
        input: &[u8],
        writer: &mut Vec<u8>,
        unknown_tokens: &mut HashSet<u16>,
    ) -> Result<(), Ck3Error> {
        let tape = BinaryTape::from_ck3(input)?;
        let mut depth = 0;
        let mut in_objects: Vec<i32> = Vec::new();
        let mut in_object = 1;
        let mut token_idx = 0;
        let mut known_number = false;
        let mut known_unquote = false;
        let mut reencode_float_token = false;
        let mut alive_data_index = 0;
        let mut unquote_list_index = 0;
        let mut ai_strategies_index = 0;
        let mut metadata_index = 0;

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
                    writer.extend_from_slice(b"{\n");
                    depth += 1;
                    in_objects.push(in_object);
                    in_object = 1;
                }
                BinaryToken::HiddenObject(_) => {
                    did_change = true;
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
                BinaryToken::End(x) => {
                    if !matches!(tokens.get(*x), Some(BinaryToken::HiddenObject(_))) {
                        writer.push(b'}');
                    }

                    if *x == alive_data_index {
                        alive_data_index = 0;
                    }

                    if *x == unquote_list_index {
                        unquote_list_index = 0;
                    }

                    if *x == ai_strategies_index {
                        ai_strategies_index = 0;
                    }

                    if *x == metadata_index {
                        metadata_index = 0;
                        if writer.len() >= 24 && &writer[0..3] == b"SAV" {
                            // If the header line is present, we will update
                            // the metadata length in bytes which is the last
                            // 8 bytes of the header line. The header line
                            // should be 24 in length
                            let new_size = format!("{:08x}", writer.len() - 24);
                            let ns = new_size.as_bytes();
                            writer[23 - ns.len()..23].copy_from_slice(ns);
                        }
                    }

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
                    if known_number || ai_strategies_index != 0 {
                        writer.extend_from_slice(format!("{}", x).as_bytes());
                        known_number = false;
                    } else if let Some(date) = Ck3Date::from_binary_heuristic(*x) {
                        writer.extend_from_slice(date.game_fmt().as_bytes());
                    } else {
                        writer.extend_from_slice(format!("{}", x).as_bytes());
                    }
                }
                BinaryToken::Quoted(x) => {
                    let data = x.view_data();
                    let end_idx = match data.last() {
                        Some(x) if *x == b'\n' => data.len() - 1,
                        Some(_x) => data.len(),
                        None => data.len(),
                    };

                    // quoted fields occuring as keys should remain unquoted
                    if in_object == 1 || known_unquote {
                        writer.extend_from_slice(&data[..end_idx]);
                    } else {
                        writer.push(b'"');
                        writer.extend_from_slice(&data[..end_idx]);
                        writer.push(b'"');
                    }
                }
                BinaryToken::Unquoted(x) => {
                    let data = x.view_data();
                    writer.extend_from_slice(&data);
                }
                BinaryToken::F32(x) => write!(writer, "{:.6}", x).map_err(Ck3ErrorKind::IoErr)?,
                BinaryToken::F64(x) if !reencode_float_token => {
                    write!(writer, "{}", x).map_err(Ck3ErrorKind::IoErr)?
                }
                BinaryToken::F64(x) => {
                    let x = reencode_float(*x);
                    if x.fract() > 1e-7 {
                        write!(writer, "{:.5}", x).map_err(Ck3ErrorKind::IoErr)?;
                    } else {
                        write!(writer, "{}", x).map_err(Ck3ErrorKind::IoErr)?;
                    }
                    reencode_float_token = false;
                }
                BinaryToken::Token(x) => match TokenLookup.resolve(*x) {
                    Some(id)
                        if self.rewrite
                            && matches!(id, "ironman" | "ironman_manager")
                            && in_object == 1 =>
                    {
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
                    Some(id) => {
                        if in_object == 1 && id == "meta_data" {
                            metadata_index = token_idx + 1;
                        }

                        if in_object == 1 && id == "alive_data" {
                            alive_data_index = token_idx + 1;
                        }

                        if in_object == 1 && id == "ai_strategies" {
                            ai_strategies_index = token_idx + 1;
                        }

                        if in_object == 1 && matches!(id, "settings" | "setting" | "perks")
                            || (id == "perk" && alive_data_index != 0)
                        {
                            unquote_list_index = token_idx + 1;
                        }

                        known_number = in_object == 1 && (id == "seed" || id == "random_count");

                        known_unquote = unquote_list_index != 0
                            || matches!(
                                id,
                                "save_game_version"
                                    | "portraits_version"
                                    | "meta_date"
                                    | "color1"
                                    | "color2"
                                    | "color3"
                                    | "color4"
                                    | "color5"
                            );

                        reencode_float_token = in_object == 1
                            && matches!(
                                id,
                                "vassal_power_value"
                                    | "budget_war_chest"
                                    | "budget_short_term"
                                    | "budget_long_term"
                                    | "budget_reserved"
                            );
                        reencode_float_token |=
                            in_object == 1 && alive_data_index != 0 && id == "gold";
                        writer.extend_from_slice(&id.as_bytes())
                    }
                    None => {
                        unknown_tokens.insert(*x);
                        match self.on_failed_resolve {
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
                        }
                    }
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
                writer.push(b'\n');
            } else if in_object != 1 {
                writer.push(b' ');
            }

            token_idx += 1;
        }

        Ok(())
    }

    /// Given one of the accepted inputs, this will return the save id line (if present in the input)
    /// with the gamestate data decoded from binary to plain text.
    pub fn melt(&self, data: &[u8]) -> Result<(Vec<u8>, HashSet<u16>), Ck3Error> {
        let mut result = Vec::with_capacity(data.len());
        let mut unknown_tokens = HashSet::new();

        // if there is a save id line in the data, we should preserve it
        let has_save_id = data.get(0..3).map_or(false, |x| x == b"SAV");
        let data = if has_save_id {
            let split_ind = data.iter().position(|&x| x == b'\n').unwrap_or(0);
            let at = std::cmp::max(split_ind, 0);
            let (header, rest) = data.split_at(at + 1);
            let mut mutted = header.to_vec();
            // Set the type to 0, type list:
            // 0: Uncompressed + Plaintext
            // 1: Uncompressed + Binary
            // 2: Compressed + Plaintext
            // 3: Compressed + Binary
            mutted["SAV010".len()] = b'0';
            result.extend_from_slice(&mutted);
            rest
        } else {
            data
        };

        let mut reader = Cursor::new(data);

        match detect_encoding(&mut reader)? {
            BodyEncoding::Plain => self.convert(data, &mut result, &mut unknown_tokens)?,
            BodyEncoding::Zip(mut zip) => {
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
                        self.convert(&inflated_data, &mut result, &mut unknown_tokens)?
                    }

                    #[cfg(feature = "mmap")]
                    Extraction::MmapTemporaries => {
                        let mut mmap = memmap::MmapMut::map_anon(zip_file.size() as usize)?;
                        std::io::copy(&mut zip_file, &mut mmap.as_mut())
                            .map_err(|e| Ck3ErrorKind::ZipExtraction("gamestate", e))?;
                        self.convert(&mmap[..], &mut result, &mut unknown_tokens)?
                    }
                }
            }
        }

        Ok((result, unknown_tokens))
    }
}
