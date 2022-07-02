use crate::{
    flavor::{flavor_from_tape, reencode_float},
    Ck3Error, Ck3ErrorKind, SaveHeader, SaveHeaderKind,
};
use jomini::{
    binary::{FailedResolveStrategy, TokenResolver},
    common::PdsDate,
    BinaryTape, BinaryToken, TextWriterBuilder,
};
use std::collections::HashSet;

#[derive(thiserror::Error, Debug)]
pub(crate) enum MelterError {
    #[error("{0}")]
    Write(#[from] jomini::Error),

    #[error("")]
    UnknownToken { token_id: u16 },
}

/// Output from melting a binary save to plaintext
pub struct MeltedDocument {
    data: Vec<u8>,
    unknown_tokens: HashSet<u16>,
}

impl MeltedDocument {
    /// The converted plaintext data
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// The list of unknown tokens that the provided resolver accumulated
    pub fn unknown_tokens(&self) -> &HashSet<u16> {
        &self.unknown_tokens
    }
}

/// Convert a binary save to plaintext
pub struct Ck3Melter<'a, 'b> {
    tape: &'b BinaryTape<'a>,
    header: &'b SaveHeader,
    verbatim: bool,
    on_failed_resolve: FailedResolveStrategy,
}

impl<'a, 'b> Ck3Melter<'a, 'b> {
    pub(crate) fn new(tape: &'b BinaryTape<'a>, header: &'b SaveHeader) -> Self {
        Ck3Melter {
            tape,
            header,
            verbatim: false,
            on_failed_resolve: FailedResolveStrategy::Ignore,
        }
    }

    pub fn verbatim(&mut self, verbatim: bool) -> &mut Self {
        self.verbatim = verbatim;
        self
    }

    pub fn on_failed_resolve(&mut self, strategy: FailedResolveStrategy) -> &mut Self {
        self.on_failed_resolve = strategy;
        self
    }

    pub(crate) fn skip_value_idx(&self, token_idx: usize) -> usize {
        self.tape
            .tokens()
            .get(token_idx + 1)
            .map(|next_token| match next_token {
                BinaryToken::Object(end) | BinaryToken::Array(end) => end + 1,
                _ => token_idx + 2,
            })
            .unwrap_or(token_idx + 1)
    }

    pub fn melt<R>(&self, resolver: &R) -> Result<MeltedDocument, Ck3Error>
    where
        R: TokenResolver,
    {
        let out = melt(self, resolver).map_err(|e| match e {
            MelterError::Write(x) => Ck3ErrorKind::Writer(x),
            MelterError::UnknownToken { token_id } => Ck3ErrorKind::UnknownToken { token_id },
        })?;
        Ok(out)
    }
}

pub(crate) fn melt<R>(melter: &Ck3Melter, resolver: &R) -> Result<MeltedDocument, MelterError>
where
    R: TokenResolver,
{
    let flavor = flavor_from_tape(melter.tape);
    let mut out = Vec::with_capacity(melter.tape.tokens().len() * 10);
    let _ = melter.header.write(&mut out);

    let mut unknown_tokens = HashSet::new();
    let mut wtr = TextWriterBuilder::new()
        .indent_char(b'\t')
        .indent_factor(1)
        .from_writer(out);
    let mut token_idx = 0;
    let mut known_number = false;
    let mut known_unquote = false;
    let mut reencode_float_token = false;
    let mut alive_data_index = 0;
    let mut unquote_list_index = 0;
    let mut ai_strategies_index = 0;
    let mut metadata_index = 0;

    // We use this to know if we are looking at a key of `ai_strategies`
    // which is always written out as a number and not a date
    let mut end_indices = Vec::new();

    let tokens = melter.tape.tokens();
    while let Some(token) = tokens.get(token_idx) {
        match token {
            BinaryToken::Object(_) => {
                end_indices.push(token_idx);
                wtr.write_object_start()?;
            }
            BinaryToken::HiddenObject(_) => {
                wtr.write_hidden_object_start()?;
            }
            BinaryToken::Array(_) => {
                end_indices.push(token_idx);
                wtr.write_array_start()?;
            }
            BinaryToken::End(x) => {
                if !matches!(tokens.get(*x), Some(BinaryToken::HiddenObject(_))) {
                    wtr.write_end()?;
                }

                end_indices.pop();
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

                    let mut new_header = melter.header.clone();
                    new_header.set_kind(SaveHeaderKind::Text);
                    new_header.set_metadata_len((data.len() + 1 - new_header.header_len()) as u64);
                    let _ = new_header.write(&mut data[..new_header.header_len()]);
                }
            }
            BinaryToken::Bool(x) => wtr.write_bool(*x)?,
            BinaryToken::U32(x) => wtr.write_u32(*x)?,
            BinaryToken::U64(x) => wtr.write_u64(*x)?,
            BinaryToken::I32(x) => {
                if known_number
                    || (end_indices
                        .last()
                        .map_or(false, |&x| x == ai_strategies_index))
                {
                    write!(wtr, "{}", x)?;
                    known_number = false;
                } else if let Some(date) = crate::Ck3Date::from_binary_heuristic(*x) {
                    wtr.write_date(date.game_fmt())?;
                } else {
                    write!(wtr, "{}", x)?;
                }
            }
            BinaryToken::Quoted(x) => {
                if known_unquote || wtr.expecting_key() {
                    wtr.write_unquoted(x.as_bytes())?;
                } else {
                    wtr.write_quoted(x.as_bytes())?;
                }
            }
            BinaryToken::Unquoted(x) => {
                wtr.write_unquoted(x.as_bytes())?;
            }
            BinaryToken::F32(x) => write!(wtr, "{:.6}", flavor.visit_f32(*x))?,
            BinaryToken::F64(x) if !reencode_float_token => {
                write!(wtr, "{}", flavor.visit_f64(*x))?;
            }
            BinaryToken::F64(x) => {
                let x = reencode_float(flavor.visit_f64(*x));
                if x.fract().abs() > 1e-6 {
                    write!(wtr, "{:.5}", x)?;
                } else {
                    write!(wtr, "{}", x)?;
                }
                reencode_float_token = false;
            }
            BinaryToken::Token(x) => match resolver.resolve(*x) {
                Some(id) => {
                    if !melter.verbatim
                        && matches!(id, "ironman" | "ironman_manager")
                        && wtr.expecting_key()
                    {
                        token_idx = melter.skip_value_idx(token_idx);
                        continue;
                    }

                    if id == "meta_data" {
                        metadata_index = token_idx + 1;
                    }

                    if id == "alive_data" {
                        alive_data_index = token_idx + 1;
                    }

                    if id == "ai_strategies" {
                        ai_strategies_index = token_idx + 1;
                    }

                    if matches!(
                        id,
                        "settings" | "setting" | "perks" | "ethnicities" | "languages"
                    ) || (id == "perk" && alive_data_index != 0)
                    {
                        unquote_list_index = token_idx + 1;
                    }

                    known_number = id == "seed" || id == "random_count";

                    known_unquote = unquote_list_index != 0 || flavor.unquote_token(id);

                    reencode_float_token = matches!(
                        id,
                        "vassal_power_value"
                            | "budget_war_chest"
                            | "budget_short_term"
                            | "budget_long_term"
                            | "budget_reserved"
                            | "damage_last_tick"
                    );
                    reencode_float_token |= alive_data_index != 0 && id == "gold";
                    reencode_float_token &= flavor.float_reencoding();
                    wtr.write_unquoted(id.as_bytes())?;
                }
                None => match melter.on_failed_resolve {
                    FailedResolveStrategy::Error => {
                        return Err(MelterError::UnknownToken { token_id: *x });
                    }
                    FailedResolveStrategy::Ignore if wtr.expecting_key() => {
                        token_idx = melter.skip_value_idx(token_idx);
                        continue;
                    }
                    _ => {
                        unknown_tokens.insert(*x);
                        write!(wtr, "__unknown_0x{:x}", x)?;
                    }
                },
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

    let mut inner = wtr.into_inner();
    inner.push(b'\n');

    Ok(MeltedDocument {
        data: inner,
        unknown_tokens,
    })
}
