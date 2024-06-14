use crate::{
    file::Ck3Zip,
    flavor::{reencode_float, Ck3BinaryFlavor, Ck3Flavor10, Ck3Flavor15},
    Ck3Error, Ck3ErrorKind, Encoding, SaveHeader, SaveHeaderKind,
};
use jomini::{
    binary::{FailedResolveStrategy, Token, TokenReader, TokenResolver},
    common::PdsDate,
    TextWriterBuilder,
};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
};

/// Output from melting a binary save to plaintext
#[derive(Debug, Default)]
pub struct MeltedDocument {
    unknown_tokens: HashSet<u16>,
}

impl MeltedDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// The list of unknown tokens that the provided resolver accumulated
    pub fn unknown_tokens(&self) -> &HashSet<u16> {
        &self.unknown_tokens
    }
}

#[derive(Debug)]
enum MeltInput<'data> {
    Text(&'data [u8]),
    Binary(&'data [u8]),
    Zip(Ck3Zip<'data>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeltOptions {
    verbatim: bool,
    on_failed_resolve: FailedResolveStrategy,
}

impl Default for MeltOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl MeltOptions {
    pub fn new() -> Self {
        Self {
            verbatim: false,
            on_failed_resolve: FailedResolveStrategy::Ignore,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum QuoteKind {
    // Regular quoting rules
    Inactive,

    // Unquote scalar and containers
    UnquoteAll,

    // Unquote only a scalar value
    UnquoteScalar,

    // Quote only a scalar value
    QuoteScalar,

    // Quote object keys
    ForceQuote,
}

#[derive(Debug, Default)]
struct Quoter {
    queued: Option<QuoteKind>,
    depth: Vec<QuoteKind>,
}

impl Quoter {
    #[inline]
    pub fn push(&mut self) {
        let next = match self.queued.take() {
            Some(x @ QuoteKind::ForceQuote | x @ QuoteKind::UnquoteAll) => x,
            _ => QuoteKind::Inactive,
        };

        self.depth.push(next);
    }

    #[inline]
    pub fn pop(&mut self) {
        let _ = self.depth.pop();
    }

    #[inline]
    pub fn take_scalar(&mut self) -> QuoteKind {
        match self.queued.take() {
            Some(x) => x,
            None => self.depth.last().copied().unwrap_or(QuoteKind::Inactive),
        }
    }

    #[inline]
    fn queue(&mut self, mode: QuoteKind) {
        self.queued = Some(mode);
    }

    #[inline]
    fn clear_queued(&mut self) {
        self.queued = None;
    }
}

#[derive(Debug, Clone, Copy)]
enum Block {
    Alive,
    AiStrategies,
    Inactive,
}

#[derive(Debug, Default)]
struct Blocks {
    queued: Option<Block>,
    data: Vec<Block>,

    in_ai_strageties: bool,
    in_alive_data: bool,
}

impl Blocks {
    #[inline]
    pub fn push(&mut self) {
        let next = self.queued.take().unwrap_or(Block::Inactive);
        self.data.push(next);
    }

    #[inline]
    fn queue(&mut self, mode: Block) {
        self.queued = Some(mode);
    }

    #[inline]
    pub fn pop(&mut self) {
        match self.data.pop() {
            Some(Block::Alive) => {
                self.in_alive_data = false;
            }
            Some(Block::AiStrategies) => {
                self.in_ai_strageties = false;
            }
            _ => {}
        }
    }

    #[inline]
    fn clear_queued(&mut self) {
        self.queued = None;
    }

    #[inline]
    fn at_ai_strategies(&self) -> bool {
        matches!(self.data.last(), Some(Block::AiStrategies))
    }
}

/// Convert a binary save to plaintext
pub struct Ck3Melter<'data> {
    input: MeltInput<'data>,
    header: SaveHeader,
    options: MeltOptions,
}

impl<'data> Ck3Melter<'data> {
    pub(crate) fn new_text(x: &'data [u8], header: SaveHeader) -> Self {
        Self {
            input: MeltInput::Text(x),
            options: MeltOptions::default(),
            header,
        }
    }

    pub(crate) fn new_binary(x: &'data [u8], header: SaveHeader) -> Self {
        Self {
            input: MeltInput::Binary(x),
            options: MeltOptions::default(),
            header,
        }
    }

    pub(crate) fn new_zip(x: Ck3Zip<'data>, header: SaveHeader) -> Self {
        Self {
            input: MeltInput::Zip(x),
            options: MeltOptions::default(),
            header,
        }
    }

    pub fn verbatim(&mut self, verbatim: bool) -> &mut Self {
        self.options.verbatim = verbatim;
        self
    }

    pub fn on_failed_resolve(&mut self, strategy: FailedResolveStrategy) -> &mut Self {
        self.options.on_failed_resolve = strategy;
        self
    }

    pub fn input_encoding(&self) -> Encoding {
        match &self.input {
            MeltInput::Text(_) => Encoding::Text,
            MeltInput::Binary(_) => Encoding::Binary,
            MeltInput::Zip(z) if z.is_text => Encoding::TextZip,
            MeltInput::Zip(_) => Encoding::BinaryZip,
        }
    }

    pub fn melt<Writer, R>(
        &mut self,
        mut output: Writer,
        resolver: &R,
    ) -> Result<MeltedDocument, Ck3Error>
    where
        Writer: Write,
        R: TokenResolver,
    {
        match &mut self.input {
            MeltInput::Text(x) => {
                self.header.write(&mut output)?;
                output.write_all(x)?;
                Ok(MeltedDocument::new())
            }
            MeltInput::Binary(x) => melt(x, output, resolver, self.options, self.header.clone()),
            MeltInput::Zip(zip) => {
                let file = zip.archive.retrieve_file(zip.gamestate);
                melt(
                    file.reader(),
                    &mut output,
                    resolver,
                    self.options,
                    self.header.clone(),
                )
            }
        }
    }
}

pub(crate) fn melt<Reader, Writer, Resolver>(
    input: Reader,
    mut output: Writer,
    resolver: Resolver,
    options: MeltOptions,
    mut header: SaveHeader,
) -> Result<MeltedDocument, Ck3Error>
where
    Reader: Read,
    Writer: Write,
    Resolver: TokenResolver,
{
    let mut reader = TokenReader::new(input);

    let header_sink = Vec::new();
    let mut wtr = TextWriterBuilder::new()
        .indent_char(b'\t')
        .indent_factor(1)
        .from_writer(Cursor::new(header_sink));

    let err = || Ck3Error::from(Ck3ErrorKind::InvalidHeader);
    match reader.next()?.ok_or_else(err)? {
        Token::Id(id) => match resolver.resolve(id) {
            Some(name) => wtr.write_unquoted(name.as_bytes())?,
            None => return Err(err()),
        },
        _ => return Err(err()),
    };

    match reader.next()?.ok_or_else(err)? {
        Token::Equal => wtr.write_operator(jomini::text::Operator::Equal)?,
        _ => return Err(err()),
    };

    match reader.next()?.ok_or_else(err)? {
        Token::Open => wtr.write_object_start()?,
        _ => return Err(err()),
    };

    match reader.next()?.ok_or_else(err)? {
        Token::Id(id) => match resolver.resolve(id) {
            Some(name) => wtr.write_unquoted(name.as_bytes())?,
            None => return Err(err()),
        },
        _ => return Err(err()),
    };

    match reader.next()?.ok_or_else(err)? {
        Token::Equal => wtr.write_operator(jomini::text::Operator::Equal)?,
        _ => return Err(err()),
    };

    let version = match reader.next()?.ok_or_else(err)? {
        Token::I32(version) => version,
        _ => return Err(err()),
    };

    wtr.write_i32(version)?;

    let flavor: Box<dyn Ck3BinaryFlavor> = if version > 5 {
        Box::new(Ck3Flavor15::new())
    } else {
        Box::new(Ck3Flavor10::new())
    };

    let mut unknown_tokens = HashSet::new();

    inner_melt(
        &mut reader,
        &mut wtr,
        &flavor,
        &resolver,
        options,
        &mut unknown_tokens,
        true,
    )?;

    let mut data = wtr.into_inner().into_inner();
    data.push(b'\n');
    header.set_kind(SaveHeaderKind::Text);
    header.set_metadata_len(data.len() as u64);

    header.write(&mut output)?;
    output.write_all(&data)?;

    let mut wtr = TextWriterBuilder::new()
        .indent_char(b'\t')
        .indent_factor(1)
        .from_writer(output);

    inner_melt(
        &mut reader,
        &mut wtr,
        &flavor,
        &resolver,
        options,
        &mut unknown_tokens,
        false,
    )?;

    Ok(MeltedDocument { unknown_tokens })
}

fn inner_melt<Reader, Writer, Resolver>(
    reader: &mut TokenReader<Reader>,
    wtr: &mut jomini::TextWriter<Writer>,
    flavor: &dyn Ck3BinaryFlavor,
    resolver: Resolver,
    options: MeltOptions,
    unknown_tokens: &mut HashSet<u16>,
    header: bool,
) -> Result<(), Ck3Error>
where
    Reader: Read,
    Writer: Write,
    Resolver: TokenResolver,
{
    let mut reencode_float_token = false;
    let mut known_number = false;
    let mut known_date = false;
    let mut quoted_buffer_enabled = false;
    let mut quoted_buffer: Vec<u8> = Vec::new();
    let mut quoter = Quoter::default();
    let mut block = Blocks::default();

    while let Some(token) = reader.next()? {
        if quoted_buffer_enabled {
            if matches!(token, Token::Equal) {
                wtr.write_unquoted(&quoted_buffer)?;
            } else {
                wtr.write_quoted(&quoted_buffer)?;
            }
            quoted_buffer.clear();
            quoted_buffer_enabled = false;
        }

        match token {
            Token::Open => {
                block.push();
                quoter.push();
                wtr.write_start()?
            }
            Token::Close => {
                block.pop();
                quoter.pop();
                wtr.write_end()?;
                if header && wtr.depth() == 0 {
                    return Ok(());
                }
            }
            Token::I32(x) => {
                if known_number || block.at_ai_strategies() {
                    wtr.write_i32(x)?;
                    known_number = false;
                } else if known_date {
                    if let Some(date) = crate::Ck3Date::from_binary(x) {
                        wtr.write_date(date.game_fmt())?;
                    } else if options.on_failed_resolve != FailedResolveStrategy::Error {
                        wtr.write_i32(x)?;
                    } else {
                        return Err(Ck3Error::new(Ck3ErrorKind::InvalidDate(x)));
                    }
                    known_date = false;
                } else if let Some(date) = crate::Ck3Date::from_binary_heuristic(x) {
                    wtr.write_date(date.game_fmt())?;
                } else {
                    wtr.write_i32(x)?;
                }
            }
            Token::Quoted(x) => match quoter.take_scalar() {
                QuoteKind::Inactive if wtr.at_unknown_start() => {
                    quoted_buffer_enabled = true;
                    quoted_buffer.extend_from_slice(x.as_bytes());
                }
                QuoteKind::Inactive if wtr.expecting_key() => wtr.write_unquoted(x.as_bytes())?,
                QuoteKind::Inactive => wtr.write_quoted(x.as_bytes())?,
                QuoteKind::ForceQuote => wtr.write_quoted(x.as_bytes())?,
                QuoteKind::UnquoteAll => wtr.write_unquoted(x.as_bytes())?,
                QuoteKind::UnquoteScalar => wtr.write_unquoted(x.as_bytes())?,
                QuoteKind::QuoteScalar => wtr.write_quoted(x.as_bytes())?,
            },
            Token::Unquoted(x) => {
                wtr.write_unquoted(x.as_bytes())?;
            }
            Token::F32(x) => write!(wtr, "{:.6}", flavor.visit_f32(x))?,
            Token::F64(x) if !reencode_float_token => write!(wtr, "{}", flavor.visit_f64(x))?,
            Token::F64(x) => {
                let x = reencode_float(flavor.visit_f64(x));
                if x.fract().abs() > 1e-6 {
                    write!(wtr, "{:.5}", x)?;
                } else {
                    write!(wtr, "{}", x)?;
                }
                reencode_float_token = false;
            }
            Token::Id(x) => match resolver.resolve(x) {
                Some(id) => {
                    if !options.verbatim
                        && matches!(id, "ironman" | "ironman_manager")
                        && wtr.expecting_key()
                    {
                        let mut next = reader.read()?;
                        if matches!(next, Token::Equal) {
                            next = reader.read()?;
                        }

                        if matches!(next, Token::Open) {
                            reader.skip_container()?;
                        }
                        continue;
                    }

                    block.clear_queued();
                    quoter.clear_queued();

                    if id == "alive_data" {
                        block.queue(Block::Alive);
                    }

                    if id == "ai_strategies" {
                        block.queue(Block::AiStrategies);
                    }

                    if matches!(
                        id,
                        "settings" | "setting" | "perks" | "ethnicities" | "languages"
                    ) {
                        quoter.queue(QuoteKind::UnquoteAll);
                    } else if id == "perk" && block.in_alive_data {
                        quoter.queue(QuoteKind::UnquoteAll);
                    } else if flavor.unquote_token(id) {
                        quoter.queue(QuoteKind::UnquoteScalar);
                    }

                    known_number = id == "seed" || id == "random_count";
                    known_date = id == "birth";
                    reencode_float_token = matches!(
                        id,
                        "vassal_power_value"
                            | "budget_war_chest"
                            | "budget_short_term"
                            | "budget_long_term"
                            | "budget_reserved"
                            | "damage_last_tick"
                    );
                    reencode_float_token |= block.in_alive_data && id == "gold";
                    reencode_float_token &= flavor.float_reencoding();

                    wtr.write_unquoted(id.as_bytes())?;
                }
                None => match options.on_failed_resolve {
                    FailedResolveStrategy::Error => {
                        return Err(Ck3ErrorKind::UnknownToken { token_id: x }.into());
                    }
                    FailedResolveStrategy::Ignore if wtr.expecting_key() => {
                        let mut next = reader.read()?;
                        if matches!(next, Token::Equal) {
                            next = reader.read()?;
                        }

                        if matches!(next, Token::Open) {
                            reader.skip_container()?;
                        }
                    }
                    _ => {
                        unknown_tokens.insert(x);
                        write!(wtr, "__unknown_0x{:x}", x)?;
                    }
                },
            },
            Token::Equal => wtr.write_operator(jomini::text::Operator::Equal)?,
            Token::U32(x) => wtr.write_u32(x)?,
            Token::U64(x) => wtr.write_u64(x)?,
            Token::Bool(x) => wtr.write_bool(x)?,
            Token::Rgb(x) => wtr.write_rgb(&x)?,
            Token::I64(x) => wtr.write_i64(x)?,
        }
    }

    Ok(())
}

// pub(crate) fn melt<R>(melter: &Ck3Melter, resolver: &R) -> Result<MeltedDocument, MelterError>
// where
//     R: TokenResolver,
// {
//     let flavor = flavor_from_tape(melter.tape);
//     let mut out = Vec::with_capacity(melter.tape.tokens().len() * 10);
//     let _ = melter.header.write(&mut out);

//     let mut unknown_tokens = HashSet::new();
//     let mut wtr = TextWriterBuilder::new()
//         .indent_char(b'\t')
//         .indent_factor(1)
//         .from_writer(out);
//     let mut token_idx = 0;
//     let mut known_number = false;
//     let mut known_unquote = false;
//     let mut known_date = false;
//     let mut reencode_float_token = false;
//     let mut alive_data_index = 0;
//     let mut unquote_list_index = 0;
//     let mut ai_strategies_index = 0;
//     let mut metadata_index = 0;

//     // We use this to know if we are looking at a key of `ai_strategies`
//     // which is always written out as a number and not a date
//     let mut end_indices = Vec::new();

//     let tokens = melter.tape.tokens();
//     while let Some(token) = tokens.get(token_idx) {
//         match token {
//             BinaryToken::Object(_) => {
//                 end_indices.push(token_idx);
//                 wtr.write_object_start()?;
//             }
//             BinaryToken::Array(_) => {
//                 end_indices.push(token_idx);
//                 wtr.write_array_start()?;
//             }
//             BinaryToken::End(x) => {
//                 wtr.write_end()?;

//                 end_indices.pop();
//                 if *x == alive_data_index {
//                     alive_data_index = 0;
//                 }

//                 if *x == unquote_list_index {
//                     unquote_list_index = 0;
//                 }

//                 if *x == ai_strategies_index {
//                     ai_strategies_index = 0;
//                 }

//                 if *x == metadata_index {
//                     metadata_index = 0;
//                     let data = wtr.inner();

//                     let mut new_header = melter.header.clone();
//                     new_header.set_kind(SaveHeaderKind::Text);
//                     new_header.set_metadata_len((data.len() + 1 - new_header.header_len()) as u64);
//                     let _ = new_header.write(&mut data[..new_header.header_len()]);
//                 }
//             }
//             BinaryToken::I32(x) => {
//                 if known_number
//                     || (end_indices
//                         .last()
//                         .map_or(false, |&x| x == ai_strategies_index))
//                 {
//                     write!(wtr, "{}", x)?;
//                     known_number = false;
//                 } else if known_date {
//                     if let Some(date) = crate::Ck3Date::from_binary(*x) {
//                         wtr.write_date(date.game_fmt())?;
//                     } else if melter.on_failed_resolve != FailedResolveStrategy::Error {
//                         wtr.write_i32(*x)?;
//                     } else {
//                         return Err(MelterError::InvalidDate(*x));
//                     }
//                     known_date = false;
//                 } else if let Some(date) = crate::Ck3Date::from_binary_heuristic(*x) {
//                     wtr.write_date(date.game_fmt())?;
//                 } else {
//                     write!(wtr, "{}", x)?;
//                 }
//             }
//             BinaryToken::Quoted(x) => {
//                 if known_unquote || wtr.expecting_key() {
//                     wtr.write_unquoted(x.as_bytes())?;
//                 } else {
//                     wtr.write_quoted(x.as_bytes())?;
//                 }
//             }
//             BinaryToken::Unquoted(x) => {
//                 wtr.write_unquoted(x.as_bytes())?;
//             }
//             BinaryToken::F32(x) => write!(wtr, "{:.6}", flavor.visit_f32(*x))?,
//             BinaryToken::F64(x) if !reencode_float_token => {
//                 write!(wtr, "{}", flavor.visit_f64(*x))?;
//             }
//             BinaryToken::F64(x) => {
//                 let x = reencode_float(flavor.visit_f64(*x));
//                 if x.fract().abs() > 1e-6 {
//                     write!(wtr, "{:.5}", x)?;
//                 } else {
//                     write!(wtr, "{}", x)?;
//                 }
//                 reencode_float_token = false;
//             }
//             BinaryToken::Token(x) => match resolver.resolve(*x) {
//                 Some(id) => {
//                     if !melter.verbatim
//                         && matches!(id, "ironman" | "ironman_manager")
//                         && wtr.expecting_key()
//                     {
//                         token_idx = melter.skip_value_idx(token_idx);
//                         continue;
//                     }

//                     if id == "meta_data" {
//                         metadata_index = token_idx + 1;
//                     }

//                     if id == "alive_data" {
//                         alive_data_index = token_idx + 1;
//                     }

//                     if id == "ai_strategies" {
//                         ai_strategies_index = token_idx + 1;
//                     }

//                     if matches!(
//                         id,
//                         "settings" | "setting" | "perks" | "ethnicities" | "languages"
//                     ) || (id == "perk" && alive_data_index != 0)
//                     {
//                         unquote_list_index = token_idx + 1;
//                     }

//                     known_number = id == "seed" || id == "random_count";
//                     known_date = id == "birth";
//                     known_unquote = unquote_list_index != 0 || flavor.unquote_token(id);

//                     reencode_float_token = matches!(
//                         id,
//                         "vassal_power_value"
//                             | "budget_war_chest"
//                             | "budget_short_term"
//                             | "budget_long_term"
//                             | "budget_reserved"
//                             | "damage_last_tick"
//                     );
//                     reencode_float_token |= alive_data_index != 0 && id == "gold";
//                     reencode_float_token &= flavor.float_reencoding();
//                     wtr.write_unquoted(id.as_bytes())?;
//                 }
//                 None => match melter.on_failed_resolve {
//                     FailedResolveStrategy::Error => {
//                         return Err(MelterError::UnknownToken { token_id: *x });
//                     }
//                     FailedResolveStrategy::Ignore if wtr.expecting_key() => {
//                         token_idx = melter.skip_value_idx(token_idx);
//                         continue;
//                     }
//                     _ => {
//                         unknown_tokens.insert(*x);
//                         write!(wtr, "__unknown_0x{:x}", x)?;
//                     }
//                 },
//             },
//             x => wtr.write_binary(x)?,
//         }

//         token_idx += 1;
//     }

//     let mut inner = wtr.into_inner();
//     inner.push(b'\n');

//     Ok(MeltedDocument {
//         data: inner,
//         unknown_tokens,
//     })
// }
