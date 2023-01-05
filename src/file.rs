use crate::{
    flavor::{flavor_from_tape, Ck3BinaryFlavor},
    Ck3Error, Ck3ErrorKind, Ck3Melter, Encoding, SaveHeader,
};
use jomini::{
    binary::{FailedResolveStrategy, TokenResolver},
    text::ObjectReader,
    BinaryDeserializer, BinaryTape, TextDeserializer, TextTape, Utf8Encoding,
};
use serde::Deserialize;
use std::io::Cursor;
use zip::result::ZipError;

enum FileKind<'a> {
    Text(&'a [u8]),
    Binary(&'a [u8]),
    Zip {
        archive: Ck3ZipFiles<'a>,
        metadata: &'a [u8],
        gamestate: VerifiedIndex,
        is_text: bool,
    },
}

/// Entrypoint for parsing CK3 saves
///
/// Only consumes enough data to determine encoding of the file
pub struct Ck3File<'a> {
    header: SaveHeader,
    kind: FileKind<'a>,
}

impl<'a> Ck3File<'a> {
    /// Creates a CK3 file from a slice of data
    pub fn from_slice(data: &[u8]) -> Result<Ck3File, Ck3Error> {
        let header = SaveHeader::from_slice(data)?;
        let data = &data[header.header_len()..];

        let reader = Cursor::new(data);
        match zip::ZipArchive::new(reader) {
            Ok(mut zip) => {
                let metadata = &data[..zip.offset() as usize];
                let files = Ck3ZipFiles::new(&mut zip, data);
                let gamestate_idx = files
                    .gamestate_index()
                    .ok_or(Ck3ErrorKind::ZipMissingEntry)?;

                let is_text = !header.kind().is_binary();
                Ok(Ck3File {
                    header,
                    kind: FileKind::Zip {
                        archive: files,
                        gamestate: gamestate_idx,
                        metadata,
                        is_text,
                    },
                })
            }
            Err(ZipError::InvalidArchive(_)) => {
                if header.kind().is_binary() {
                    Ok(Ck3File {
                        header,
                        kind: FileKind::Binary(data),
                    })
                } else {
                    Ok(Ck3File {
                        header,
                        kind: FileKind::Text(data),
                    })
                }
            }
            Err(e) => Err(Ck3ErrorKind::ZipArchive(e).into()),
        }
    }

    /// Return first line header
    pub fn header(&self) -> &SaveHeader {
        &self.header
    }

    /// Returns the detected decoding of the file
    pub fn encoding(&self) -> Encoding {
        match &self.kind {
            FileKind::Text(_) => Encoding::Text,
            FileKind::Binary(_) => Encoding::Binary,
            FileKind::Zip { is_text: true, .. } => Encoding::TextZip,
            FileKind::Zip { is_text: false, .. } => Encoding::BinaryZip,
        }
    }

    /// Returns the size of the file
    ///
    /// The size includes the inflated size of the zip
    pub fn size(&self) -> usize {
        match &self.kind {
            FileKind::Text(x) | FileKind::Binary(x) => x.len(),
            FileKind::Zip { gamestate, .. } => gamestate.size,
        }
    }

    pub fn meta(&self) -> Ck3Meta<'a> {
        match &self.kind {
            FileKind::Text(x) => {
                // The metadata section should be way smaller than the total
                // length so if the total data isn't significantly bigger (2x or
                // more), assume that the header doesn't accurately represent
                // the metadata length. Like maybe someone accidentally
                // converted the line endings from unix to dos.
                let len = self.header.metadata_len() as usize;
                let data = if len * 2 > x.len() { x } else { &x[..len] };
                Ck3Meta {
                    kind: Ck3MetaKind::Text(data),
                    header: self.header.clone(),
                }
            }
            FileKind::Binary(x) => {
                let metadata = x.get(..self.header.metadata_len() as usize).unwrap_or(x);
                Ck3Meta {
                    kind: Ck3MetaKind::Binary(metadata),
                    header: self.header.clone(),
                }
            }
            FileKind::Zip {
                metadata,
                is_text: true,
                ..
            } => Ck3Meta {
                kind: Ck3MetaKind::Text(metadata),
                header: self.header.clone(),
            },
            FileKind::Zip { metadata, .. } => Ck3Meta {
                kind: Ck3MetaKind::Binary(metadata),
                header: self.header.clone(),
            },
        }
    }

    /// Parses the entire file
    ///
    /// If the file is a zip, the zip contents will be inflated into the zip
    /// sink before being parsed
    pub fn parse(&self, zip_sink: &'a mut Vec<u8>) -> Result<Ck3ParsedFile<'a>, Ck3Error> {
        match &self.kind {
            FileKind::Text(x) => {
                let text = Ck3Text::from_raw(x)?;
                Ok(Ck3ParsedFile {
                    kind: Ck3ParsedFileKind::Text(text),
                })
            }
            FileKind::Binary(x) => {
                let binary = Ck3Binary::from_raw(x, self.header.clone())?;
                Ok(Ck3ParsedFile {
                    kind: Ck3ParsedFileKind::Binary(binary),
                })
            }
            FileKind::Zip {
                archive,
                gamestate,
                is_text,
                ..
            } => {
                let zip = archive.retrieve_file(*gamestate);
                zip.read_to_end(zip_sink)?;

                if *is_text {
                    let text = Ck3Text::from_raw(zip_sink)?;
                    Ok(Ck3ParsedFile {
                        kind: Ck3ParsedFileKind::Text(text),
                    })
                } else {
                    let binary = Ck3Binary::from_raw(zip_sink, self.header.clone())?;
                    Ok(Ck3ParsedFile {
                        kind: Ck3ParsedFileKind::Binary(binary),
                    })
                }
            }
        }
    }
}

pub struct Ck3Meta<'a> {
    kind: Ck3MetaKind<'a>,
    header: SaveHeader,
}

enum Ck3MetaKind<'a> {
    Text(&'a [u8]),
    Binary(&'a [u8]),
}

impl<'a> Ck3Meta<'a> {
    pub fn data(&self) -> &'a [u8] {
        match self.kind {
            Ck3MetaKind::Text(x) | Ck3MetaKind::Binary(x) => x,
        }
    }

    pub fn is_text(&self) -> bool {
        self.header.kind().is_text()
    }

    pub fn is_binary(&self) -> bool {
        self.header.kind().is_binary()
    }

    pub fn parse(&self) -> Result<Ck3ParsedFile, Ck3Error> {
        match self.kind {
            Ck3MetaKind::Text(x) => Ck3Text::from_raw(x).map(|kind| Ck3ParsedFile {
                kind: Ck3ParsedFileKind::Text(kind),
            }),

            Ck3MetaKind::Binary(x) => {
                Ck3Binary::from_raw(x, self.header.clone()).map(|kind| Ck3ParsedFile {
                    kind: Ck3ParsedFileKind::Binary(kind),
                })
            }
        }
    }
}

/// Contains the parsed Ck3 file
pub enum Ck3ParsedFileKind<'a> {
    /// The Ck3 file as text
    Text(Ck3Text<'a>),

    /// The Ck3 file as binary
    Binary(Ck3Binary<'a>),
}

/// An Ck3 file that has been parsed
pub struct Ck3ParsedFile<'a> {
    kind: Ck3ParsedFileKind<'a>,
}

impl<'a> Ck3ParsedFile<'a> {
    /// Returns the file as text
    pub fn as_text(&self) -> Option<&Ck3Text> {
        match &self.kind {
            Ck3ParsedFileKind::Text(x) => Some(x),
            _ => None,
        }
    }

    /// Returns the file as binary
    pub fn as_binary(&self) -> Option<&Ck3Binary> {
        match &self.kind {
            Ck3ParsedFileKind::Binary(x) => Some(x),
            _ => None,
        }
    }

    /// Returns the kind of file (binary or text)
    pub fn kind(&self) -> &Ck3ParsedFileKind {
        &self.kind
    }

    /// Prepares the file for deserialization into a custom structure
    pub fn deserializer<'b, RES>(&'b self, resolver: &'b RES) -> Ck3Deserializer<RES>
    where
        RES: TokenResolver,
    {
        match &self.kind {
            Ck3ParsedFileKind::Text(x) => Ck3Deserializer {
                kind: Ck3DeserializerKind::Text(x),
            },
            Ck3ParsedFileKind::Binary(x) => Ck3Deserializer {
                kind: Ck3DeserializerKind::Binary(x.deserializer(resolver)),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct VerifiedIndex {
    data_start: usize,
    data_end: usize,
    size: usize,
}

#[derive(Debug, Clone)]
struct Ck3ZipFiles<'a> {
    archive: &'a [u8],
    gamestate_index: Option<VerifiedIndex>,
}

impl<'a> Ck3ZipFiles<'a> {
    pub fn new(archive: &mut zip::ZipArchive<Cursor<&'a [u8]>>, data: &'a [u8]) -> Self {
        let mut gamestate_index = None;

        for index in 0..archive.len() {
            if let Ok(file) = archive.by_index_raw(index) {
                let size = file.size() as usize;
                let data_start = file.data_start() as usize;
                let data_end = data_start + file.compressed_size() as usize;

                if file.name() == "gamestate" {
                    gamestate_index = Some(VerifiedIndex {
                        data_start,
                        data_end,
                        size,
                    })
                }
            }
        }

        Self {
            archive: data,
            gamestate_index,
        }
    }

    pub fn retrieve_file(&self, index: VerifiedIndex) -> Ck3ZipFile {
        let raw = &self.archive[index.data_start..index.data_end];
        Ck3ZipFile {
            raw,
            size: index.size,
        }
    }

    pub fn gamestate_index(&self) -> Option<VerifiedIndex> {
        self.gamestate_index
    }
}

struct Ck3ZipFile<'a> {
    raw: &'a [u8],
    size: usize,
}

impl<'a> Ck3ZipFile<'a> {
    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> Result<(), Ck3Error> {
        let start_len = buf.len();
        buf.resize(start_len + self.size(), 0);
        let body = &mut buf[start_len..];
        crate::deflate::inflate_exact(self.raw, body).map_err(Ck3ErrorKind::from)?;
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

/// A parsed Ck3 text document
pub struct Ck3Text<'a> {
    tape: TextTape<'a>,
}

impl<'a> Ck3Text<'a> {
    pub fn from_slice(data: &'a [u8]) -> Result<Self, Ck3Error> {
        let header = SaveHeader::from_slice(data)?;
        Self::from_raw(&data[..header.header_len()])
    }

    pub(crate) fn from_raw(data: &'a [u8]) -> Result<Self, Ck3Error> {
        let tape = TextTape::from_slice(data).map_err(Ck3ErrorKind::Parse)?;
        Ok(Ck3Text { tape })
    }

    pub fn reader(&self) -> ObjectReader<Utf8Encoding> {
        self.tape.utf8_reader()
    }

    pub fn deserialize<T>(&self) -> Result<T, Ck3Error>
    where
        T: Deserialize<'a>,
    {
        let deser = TextDeserializer::from_utf8_tape(&self.tape);
        let result = deser.deserialize().map_err(Ck3ErrorKind::Deserialize)?;
        Ok(result)
    }
}

/// A parsed Ck3 binary document
pub struct Ck3Binary<'a> {
    tape: BinaryTape<'a>,
    header: SaveHeader,
}

impl<'a> Ck3Binary<'a> {
    pub fn from_slice(data: &'a [u8]) -> Result<Self, Ck3Error> {
        let header = SaveHeader::from_slice(data)?;
        Self::from_raw(&data[..header.header_len()], header)
    }

    pub(crate) fn from_raw(data: &'a [u8], header: SaveHeader) -> Result<Self, Ck3Error> {
        let tape = BinaryTape::from_slice(data).map_err(Ck3ErrorKind::Parse)?;
        Ok(Ck3Binary { tape, header })
    }

    pub fn deserializer<'b, RES>(&'b self, resolver: &'b RES) -> Ck3BinaryDeserializer<RES>
    where
        RES: TokenResolver,
    {
        Ck3BinaryDeserializer {
            deser: BinaryDeserializer::builder_flavor(flavor_from_tape(&self.tape))
                .from_tape(&self.tape, resolver),
        }
    }

    pub fn melter<'b>(&'b self) -> Ck3Melter<'a, 'b> {
        Ck3Melter::new(&self.tape, &self.header)
    }
}

enum Ck3DeserializerKind<'data, 'tape, RES> {
    Text(&'tape Ck3Text<'data>),
    Binary(Ck3BinaryDeserializer<'data, 'tape, RES>),
}

/// A deserializer for custom structures
pub struct Ck3Deserializer<'data, 'tape, RES> {
    kind: Ck3DeserializerKind<'data, 'tape, RES>,
}

impl<'data, 'tape, RES> Ck3Deserializer<'data, 'tape, RES>
where
    RES: TokenResolver,
{
    pub fn on_failed_resolve(&mut self, strategy: FailedResolveStrategy) -> &mut Self {
        if let Ck3DeserializerKind::Binary(x) = &mut self.kind {
            x.on_failed_resolve(strategy);
        }
        self
    }

    pub fn deserialize<T>(&self) -> Result<T, Ck3Error>
    where
        T: Deserialize<'data>,
    {
        match &self.kind {
            Ck3DeserializerKind::Text(x) => x.deserialize(),
            Ck3DeserializerKind::Binary(x) => x.deserialize(),
        }
    }
}

/// Deserializes binary data into custom structures
pub struct Ck3BinaryDeserializer<'data, 'tape, RES> {
    deser: BinaryDeserializer<'tape, 'data, 'tape, RES, Box<dyn Ck3BinaryFlavor>>,
}

impl<'data, 'tape, RES> Ck3BinaryDeserializer<'data, 'tape, RES>
where
    RES: TokenResolver,
{
    pub fn on_failed_resolve(&mut self, strategy: FailedResolveStrategy) -> &mut Self {
        self.deser.on_failed_resolve(strategy);
        self
    }

    pub fn deserialize<T>(&self) -> Result<T, Ck3Error>
    where
        T: Deserialize<'data>,
    {
        let result = self.deser.deserialize().map_err(|e| match e.kind() {
            jomini::ErrorKind::Deserialize(e2) => match e2.kind() {
                &jomini::DeserializeErrorKind::UnknownToken { token_id } => {
                    Ck3ErrorKind::UnknownToken { token_id }
                }
                _ => Ck3ErrorKind::Deserialize(e),
            },
            _ => Ck3ErrorKind::Deserialize(e),
        })?;
        Ok(result)
    }
}
