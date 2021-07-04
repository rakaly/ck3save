use crate::util::reencode_float;
use crate::{
    detect_encoding, tokens::TokenLookup, BodyEncoding, Ck3Date, Ck3Error, Ck3ErrorKind,
    Extraction, FailedResolveStrategy, PdsDate
};
use jomini::{BinaryTape, BinaryToken, TextWriterBuilder, TokenResolver, WriteVisitor};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
};

struct Ck3Visitor;
impl WriteVisitor for Ck3Visitor {
    fn visit_f32<W>(&self, mut writer: W, data: f32) -> Result<(), jomini::Error>
    where
        W: Write,
    {
        write!(writer, "{:.6}", data).map_err(|e| e.into())
    }

    fn visit_f64<W>(&self, mut writer: W, data: f64) -> Result<(), jomini::Error>
    where
        W: Write,
    {
        write!(writer, "{:.5}", data).map_err(|e| e.into())
    }
}

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
        let mut wtr = TextWriterBuilder::new()
            .indent_char(b'\t')
            .indent_factor(1).from_writer_visitor(writer, Ck3Visitor);
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
            match token {
                BinaryToken::Object(_) => {
                    wtr.write_object_start()?;
                }
                BinaryToken::HiddenObject(_) => {
                    wtr.write_hidden_object_start()?;
                }
                BinaryToken::Array(_) => {
                    wtr.write_array_start()?;
                }
                BinaryToken::End(x) => {
                    if !matches!(tokens.get(*x), Some(BinaryToken::HiddenObject(_))) {
                        wtr.write_end()?;
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
                        let data = wtr.inner();

                        if data.len() >= 24 && &data[0..3] == b"SAV" {
                            // If the header line is present, we will update
                            // the metadata length in bytes which is the last
                            // 8 bytes of the header line. The header line
                            // should be 24 in length
                            let new_size = format!("{:08x}", data.len() - 24);
                            let ns = new_size.as_bytes();
                            data[23 - ns.len()..23].copy_from_slice(ns);
                        }
                    }
                }
                BinaryToken::Bool(x) => wtr.write_bool(*x)?,
                BinaryToken::U32(x) => wtr.write_u32(*x)?,
                BinaryToken::U64(x) => wtr.write_u64(*x)?,
                BinaryToken::I32(x) => {
                    if known_number || ai_strategies_index != 0 {
                        write!(wtr, "{}", x)?;
                        known_number = false;
                    } else if let Some(date) = Ck3Date::from_binary_heuristic(*x) {
                        wtr.write_date(date.game_fmt())?;
                    } else {
                        write!(wtr, "{}", x)?;
                    }
                }
                BinaryToken::Quoted(x) => {
                    if known_unquote {
                        wtr.write_unquoted(x.as_bytes())?;
                    } else {
                        wtr.write_quoted(x.as_bytes())?;
                    }
                }
                BinaryToken::Unquoted(x) => {
                    wtr.write_unquoted(x.as_bytes())?;
                }
                BinaryToken::F32(x) => wtr.write_f32(*x)?,
                BinaryToken::F64(x) if !reencode_float_token => {
                    write!(wtr, "{}", x)?;
                }
                BinaryToken::F64(x) => {
                    let x = reencode_float(*x);
                    if x.fract() > 1e-7 {
                        write!(wtr, "{:.5}", x)?;
                    } else {
                        write!(wtr, "{}", x)?;
                    }
                    reencode_float_token = false;
                }
                BinaryToken::Token(x) => match TokenLookup.resolve(*x) {
                    Some(id)
                        if self.rewrite
                            && matches!(id, "ironman" | "ironman_manager")
                            && wtr.expecting_key() =>
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
                        if id == "meta_data" {
                            metadata_index = token_idx + 1;
                        }

                        if id == "alive_data" {
                            alive_data_index = token_idx + 1;
                        }

                        if id == "ai_strategies" {
                            ai_strategies_index = token_idx + 1;
                        }

                        if matches!(id, "settings" | "setting" | "perks")
                            || (id == "perk" && alive_data_index != 0)
                        {
                            unquote_list_index = token_idx + 1;
                        }

                        known_number = id == "seed" || id == "random_count";

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

                        reencode_float_token = matches!(
                            id,
                            "vassal_power_value"
                                | "budget_war_chest"
                                | "budget_short_term"
                                | "budget_long_term"
                                | "budget_reserved"
                        );
                        reencode_float_token |= alive_data_index != 0 && id == "gold";
                        wtr.write_unquoted(id.as_bytes())?;
                    }
                    None => {
                        unknown_tokens.insert(*x);
                        match self.on_failed_resolve {
                            FailedResolveStrategy::Error => {
                                return Err(Ck3ErrorKind::UnknownToken { token_id: *x }.into());
                            }
                            FailedResolveStrategy::Ignore if wtr.expecting_key() => {
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
                                write!(wtr, "__unknown_0x{:x}", x)?;
                            }
                        }
                    }
                },
                BinaryToken::Rgb(color) => {
                    wtr.write_header(b"rgb")?;
                    wtr.write_array_start()?;
                    wtr.write_u32(color.r)?;
                    wtr.write_u32(color.g)?;
                    wtr.write_u32(color.b)?;
                    wtr.write_end()?;
                }
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
