use crate::{
    flavor::flavor_reader, melt, models::Gamestate, Ck3Error, Ck3ErrorKind, Encoding, MeltOptions,
    MeltedDocument, SaveHeader,
};
use jomini::{binary::TokenResolver, text::ObjectReader, TextDeserializer, TextTape, Utf8Encoding};
use rawzip::{FileReader, ReaderAt, ZipArchiveEntryWayfinder, ZipVerifier};
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read, Seek, Write},
    ops::Range,
};

/// Entrypoint for parsing CK3 saves
///
/// Only consumes enough data to determine encoding of the file
pub struct Ck3File {}

impl Ck3File {
    /// Creates a CK3 file from a slice of data
    pub fn from_slice(data: &[u8]) -> Result<Ck3SliceFile<'_>, Ck3Error> {
        let header = SaveHeader::from_slice(data)?;
        let data = &data[header.header_len()..];

        let archive = rawzip::ZipArchive::with_max_search_space(64 * 1024)
            .locate_in_slice(data)
            .map_err(|(_, e)| Ck3ErrorKind::Zip(e));

        match archive {
            Ok(archive) => {
                let archive = archive.into_reader();
                let mut buf = vec![0u8; rawzip::RECOMMENDED_BUFFER_SIZE];
                let zip = Ck3Zip::try_from_archive(archive, &mut buf, header.clone())?;
                Ok(Ck3SliceFile {
                    header,
                    kind: Ck3SliceFileKind::Zip(Box::new(zip)),
                })
            }
            _ if header.kind().is_binary() => Ok(Ck3SliceFile {
                header: header.clone(),
                kind: Ck3SliceFileKind::Binary(Ck3Binary {
                    reader: data,
                    header,
                }),
            }),
            _ => Ok(Ck3SliceFile {
                header,
                kind: Ck3SliceFileKind::Text(Ck3Text(data)),
            }),
        }
    }

    pub fn from_file(mut file: File) -> Result<Ck3FsFile<FileReader>, Ck3Error> {
        let mut buf = [0u8; SaveHeader::SIZE];
        file.read_exact(&mut buf)?;
        let header = SaveHeader::from_slice(&buf)?;
        let mut buf = vec![0u8; rawzip::RECOMMENDED_BUFFER_SIZE];

        let archive =
            rawzip::ZipArchive::with_max_search_space(64 * 1024).locate_in_file(file, &mut buf);

        match archive {
            Ok(archive) => {
                let zip = Ck3Zip::try_from_archive(archive, &mut buf, header.clone())?;
                Ok(Ck3FsFile {
                    header,
                    kind: Ck3FsFileKind::Zip(Box::new(zip)),
                })
            }
            Err((mut file, _)) => {
                file.seek(std::io::SeekFrom::Start(SaveHeader::SIZE as u64))?;
                if header.kind().is_binary() {
                    Ok(Ck3FsFile {
                        header: header.clone(),
                        kind: Ck3FsFileKind::Binary(Ck3Binary {
                            header,
                            reader: file,
                        }),
                    })
                } else {
                    Ok(Ck3FsFile {
                        header,
                        kind: Ck3FsFileKind::Text(file),
                    })
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Ck3SliceFileKind<'a> {
    Text(Ck3Text<'a>),
    Binary(Ck3Binary<&'a [u8]>),
    Zip(Box<Ck3Zip<&'a [u8]>>),
}

#[derive(Debug, Clone)]
pub struct Ck3SliceFile<'a> {
    header: SaveHeader,
    kind: Ck3SliceFileKind<'a>,
}

impl<'a> Ck3SliceFile<'a> {
    pub fn kind(&self) -> &Ck3SliceFileKind<'a> {
        &self.kind
    }

    pub fn kind_mut(&'a mut self) -> &'a mut Ck3SliceFileKind<'a> {
        &mut self.kind
    }

    pub fn encoding(&self) -> Encoding {
        match &self.kind {
            Ck3SliceFileKind::Text(_) => Encoding::Text,
            Ck3SliceFileKind::Binary(_) => Encoding::Binary,
            Ck3SliceFileKind::Zip(_) if self.header.kind().is_text() => Encoding::TextZip,
            Ck3SliceFileKind::Zip(_) => Encoding::BinaryZip,
        }
    }

    pub fn parse_save<R>(&self, resolver: R) -> Result<Gamestate, Ck3Error>
    where
        R: TokenResolver,
    {
        match &self.kind {
            Ck3SliceFileKind::Text(data) => data.deserializer().deserialize(),
            Ck3SliceFileKind::Binary(data) => data.clone().deserializer(resolver).deserialize(),
            Ck3SliceFileKind::Zip(archive) => {
                let game: Gamestate = archive.deserialize_gamestate(&resolver)?;
                Ok(game)
            }
        }
    }

    pub fn melt<Resolver, Writer>(
        &self,
        options: MeltOptions,
        resolver: Resolver,
        mut output: Writer,
    ) -> Result<MeltedDocument, Ck3Error>
    where
        Resolver: TokenResolver,
        Writer: Write,
    {
        match &self.kind {
            Ck3SliceFileKind::Text(data) => {
                let mut new_header = self.header.clone();
                new_header.set_kind(crate::SaveHeaderKind::Text);
                new_header.write(&mut output)?;
                output.write_all(data.0)?;
                Ok(MeltedDocument::new())
            }
            Ck3SliceFileKind::Binary(data) => data.clone().melt(options, resolver, output),
            Ck3SliceFileKind::Zip(zip) => zip.melt(options, resolver, output),
        }
    }
}

pub enum Ck3FsFileKind<R> {
    Text(File),
    Binary(Ck3Binary<File>),
    Zip(Box<Ck3Zip<R>>),
}

pub struct Ck3FsFile<R> {
    header: SaveHeader,
    kind: Ck3FsFileKind<R>,
}

impl<R> Ck3FsFile<R> {
    pub fn kind(&self) -> &Ck3FsFileKind<R> {
        &self.kind
    }

    pub fn kind_mut(&mut self) -> &mut Ck3FsFileKind<R> {
        &mut self.kind
    }

    pub fn encoding(&self) -> Encoding {
        match &self.kind {
            Ck3FsFileKind::Text(_) => Encoding::Text,
            Ck3FsFileKind::Binary(_) => Encoding::Binary,
            Ck3FsFileKind::Zip(_) if self.header.kind().is_text() => Encoding::TextZip,
            Ck3FsFileKind::Zip(_) => Encoding::BinaryZip,
        }
    }
}

impl<R> Ck3FsFile<R>
where
    R: ReaderAt,
{
    pub fn parse_save<RES>(&mut self, resolver: RES) -> Result<Gamestate, Ck3Error>
    where
        RES: TokenResolver,
    {
        match &mut self.kind {
            Ck3FsFileKind::Text(file) => {
                let reader = jomini::text::TokenReader::new(file);
                let mut deserializer = TextDeserializer::from_utf8_reader(reader);
                Ok(deserializer.deserialize()?)
            }
            Ck3FsFileKind::Binary(file) => {
                let result = file.deserializer(resolver).deserialize()?;
                Ok(result)
            }
            Ck3FsFileKind::Zip(archive) => {
                let game: Gamestate = archive.deserialize_gamestate(resolver)?;
                Ok(game)
            }
        }
    }

    pub fn melt<Resolver, Writer>(
        &mut self,
        options: MeltOptions,
        resolver: Resolver,
        mut output: Writer,
    ) -> Result<MeltedDocument, Ck3Error>
    where
        Resolver: TokenResolver,
        Writer: Write,
    {
        match &mut self.kind {
            Ck3FsFileKind::Text(file) => {
                let mut new_header = self.header.clone();
                new_header.set_kind(crate::SaveHeaderKind::Text);
                new_header.write(&mut output)?;
                std::io::copy(file, &mut output)?;
                Ok(MeltedDocument::new())
            }
            Ck3FsFileKind::Binary(data) => data.melt(options, resolver, output),
            Ck3FsFileKind::Zip(zip) => zip.melt(options, resolver, output),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ck3Zip<R> {
    pub(crate) archive: rawzip::ZipArchive<R>,
    pub(crate) metadata: Ck3MetaKind,
    pub(crate) gamestate: ZipArchiveEntryWayfinder,
    pub(crate) header: SaveHeader,
}

impl<R> Ck3Zip<R>
where
    R: ReaderAt,
{
    pub fn try_from_archive(
        archive: rawzip::ZipArchive<R>,
        buf: &mut [u8],
        header: SaveHeader,
    ) -> Result<Self, Ck3Error> {
        let mut offset = archive.directory_offset();
        let mut entries = archive.entries(buf);
        let mut gamestate = None;
        let mut metadata = None;

        while let Some(entry) = entries.next_entry().map_err(Ck3ErrorKind::Zip)? {
            offset = offset.min(entry.local_header_offset());
            match entry.file_path().as_ref() {
                b"gamestate" => gamestate = Some(entry.wayfinder()),
                b"meta" => metadata = Some(entry.wayfinder()),
                _ => {}
            };
        }

        match (gamestate, metadata) {
            (Some(gamestate), Some(metadata)) => Ok(Ck3Zip {
                archive,
                gamestate,
                metadata: Ck3MetaKind::Zip(metadata),
                header,
            }),
            (Some(gamestate), None) => Ok(Ck3Zip {
                archive,
                gamestate,
                metadata: Ck3MetaKind::Inlined(SaveHeader::SIZE..offset as usize),
                header,
            }),
            _ => Err(Ck3ErrorKind::ZipMissingEntry.into()),
        }
    }

    pub fn deserialize_gamestate<T, RES>(&self, resolver: RES) -> Result<T, Ck3Error>
    where
        T: DeserializeOwned,
        RES: TokenResolver,
    {
        let zip_entry = self
            .archive
            .get_entry(self.gamestate)
            .map_err(Ck3ErrorKind::Zip)?;
        let reader = CompressedFileReader::from_compressed(zip_entry.reader())?;
        let reader = zip_entry.verifying_reader(reader);
        let encoding = if self.header.kind().is_binary() {
            Encoding::Binary
        } else {
            Encoding::Text
        };
        let data: T = Ck3Modeller::from_reader(reader, &resolver, encoding).deserialize()?;
        Ok(data)
    }

    pub fn meta(&self) -> Result<Ck3Entry<rawzip::ZipReader<&R>, &R>, Ck3Error> {
        let kind = match &self.metadata {
            Ck3MetaKind::Inlined(x) => {
                let mut entry = vec![0u8; x.len()];
                self.archive
                    .get_ref()
                    .read_exact_at(&mut entry, x.start as u64)?;
                Ck3EntryKind::Inlined(Cursor::new(entry))
            }
            Ck3MetaKind::Zip(wayfinder) => {
                let zip_entry = self
                    .archive
                    .get_entry(*wayfinder)
                    .map_err(Ck3ErrorKind::Zip)?;
                let reader = CompressedFileReader::from_compressed(zip_entry.reader())?;
                let reader = zip_entry.verifying_reader(reader);
                Ck3EntryKind::Zip(reader)
            }
        };

        Ok(Ck3Entry {
            inner: kind,
            header: self.header.clone(),
        })
    }

    pub fn melt<Resolver, Writer>(
        &self,
        options: MeltOptions,
        resolver: Resolver,
        mut output: Writer,
    ) -> Result<MeltedDocument, Ck3Error>
    where
        Resolver: TokenResolver,
        Writer: Write,
    {
        let zip_entry = self
            .archive
            .get_entry(self.gamestate)
            .map_err(Ck3ErrorKind::Zip)?;
        let reader = CompressedFileReader::from_compressed(zip_entry.reader())?;
        let mut reader = zip_entry.verifying_reader(reader);

        if self.header.kind().is_text() {
            let mut new_header = self.header.clone();
            new_header.set_kind(crate::SaveHeaderKind::Text);
            new_header.write(&mut output)?;
            std::io::copy(&mut reader, &mut output)?;
            Ok(MeltedDocument::new())
        } else {
            melt::melt(
                &mut reader,
                &mut output,
                resolver,
                options,
                self.header.clone(),
            )
        }
    }
}

/// Describes the format of the metadata section of the save
#[derive(Debug, Clone)]
pub enum Ck3MetaKind {
    Inlined(Range<usize>),
    Zip(ZipArchiveEntryWayfinder),
}

#[derive(Debug)]
pub struct Ck3Entry<R, ReadAt> {
    inner: Ck3EntryKind<R, ReadAt>,
    header: SaveHeader,
}

#[derive(Debug)]
pub enum Ck3EntryKind<R, ReadAt> {
    Inlined(Cursor<Vec<u8>>),
    Zip(ZipVerifier<CompressedFileReader<R>, ReadAt>),
}

impl<R, ReadAt> Read for Ck3Entry<R, ReadAt>
where
    R: Read,
    ReadAt: ReaderAt,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            Ck3EntryKind::Inlined(data) => data.read(buf),
            Ck3EntryKind::Zip(reader) => reader.read(buf),
        }
    }
}

impl<R, ReadAt> Ck3Entry<R, ReadAt>
where
    R: Read,
    ReadAt: ReaderAt,
{
    pub fn deserializer<'a, RES>(
        &'a mut self,
        resolver: RES,
    ) -> Ck3Modeller<&'a mut Ck3Entry<R, ReadAt>, RES>
    where
        RES: TokenResolver,
    {
        let encoding = if self.header.kind().is_text() {
            Encoding::Text
        } else {
            Encoding::Binary
        };
        Ck3Modeller::from_reader(self, resolver, encoding)
    }

    pub fn melt<Resolver, Writer>(
        &mut self,
        options: MeltOptions,
        resolver: Resolver,
        mut output: Writer,
    ) -> Result<MeltedDocument, Ck3Error>
    where
        Resolver: TokenResolver,
        Writer: Write,
    {
        if self.header.kind().is_text() {
            let mut new_header = self.header.clone();
            new_header.set_kind(crate::SaveHeaderKind::Text);
            new_header.write(&mut output)?;
            std::io::copy(self, &mut output)?;
            Ok(MeltedDocument::new())
        } else {
            let header = self.header.clone();
            melt::melt(self, &mut output, resolver, options, header)
        }
    }
}

/// A parsed Ck3 text document
pub struct Ck3ParsedText<'a> {
    tape: TextTape<'a>,
}

impl<'a> Ck3ParsedText<'a> {
    pub fn from_slice(data: &'a [u8]) -> Result<Self, Ck3Error> {
        let header = SaveHeader::from_slice(data)?;
        Self::from_raw(&data[header.header_len()..])
    }

    pub fn from_raw(data: &'a [u8]) -> Result<Self, Ck3Error> {
        let tape = TextTape::from_slice(data).map_err(Ck3ErrorKind::Parse)?;
        Ok(Ck3ParsedText { tape })
    }

    pub fn reader(&self) -> ObjectReader<'_, '_, Utf8Encoding> {
        self.tape.utf8_reader()
    }
}

#[derive(Debug, Clone)]
pub struct Ck3Text<'a>(&'a [u8]);

impl Ck3Text<'_> {
    pub fn get_ref(&self) -> &[u8] {
        self.0
    }

    pub fn deserializer(&self) -> Ck3Modeller<&[u8], HashMap<u16, String>> {
        Ck3Modeller {
            reader: self.0,
            resolver: HashMap::new(),
            encoding: Encoding::Text,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ck3Binary<R> {
    reader: R,
    header: SaveHeader,
}

impl<R> Ck3Binary<R>
where
    R: Read,
{
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    pub fn deserializer<RES>(&mut self, resolver: RES) -> Ck3Modeller<&'_ mut R, RES> {
        Ck3Modeller {
            reader: &mut self.reader,
            resolver,
            encoding: Encoding::Binary,
        }
    }

    pub fn melt<Resolver, Writer>(
        &mut self,
        options: MeltOptions,
        resolver: Resolver,
        mut output: Writer,
    ) -> Result<MeltedDocument, Ck3Error>
    where
        Resolver: TokenResolver,
        Writer: Write,
    {
        melt::melt(
            &mut self.reader,
            &mut output,
            resolver,
            options,
            self.header.clone(),
        )
    }
}

#[derive(Debug)]
pub struct Ck3Modeller<Reader, Resolver> {
    reader: Reader,
    resolver: Resolver,
    encoding: Encoding,
}

impl<Reader: Read, Resolver: TokenResolver> Ck3Modeller<Reader, Resolver> {
    pub fn from_reader(reader: Reader, resolver: Resolver, encoding: Encoding) -> Self {
        Ck3Modeller {
            reader,
            resolver,
            encoding,
        }
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    pub fn deserialize<T>(&mut self) -> Result<T, Ck3Error>
    where
        T: DeserializeOwned,
    {
        T::deserialize(self)
    }

    pub fn into_inner(self) -> Reader {
        self.reader
    }
}

impl<'de, 'a: 'de, Reader: Read, Resolver: TokenResolver> serde::de::Deserializer<'de>
    for &'a mut Ck3Modeller<Reader, Resolver>
{
    type Error = Ck3Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Ck3Error::new(Ck3ErrorKind::DeserializeImpl {
            msg: String::from("only struct supported"),
        }))
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if matches!(self.encoding, Encoding::Binary) {
            use jomini::binary::BinaryFlavor;
            let (reader, flavor) = flavor_reader(&mut self.reader)?;
            let mut deser = flavor.deserializer().from_reader(reader, &self.resolver);
            Ok(deser.deserialize_struct(name, fields, visitor)?)
        } else {
            let reader = jomini::text::TokenReader::new(&mut self.reader);
            let mut deser = TextDeserializer::from_utf8_reader(reader);
            Ok(deser.deserialize_struct(name, fields, visitor)?)
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

#[derive(Debug)]
pub struct CompressedFileReader<R> {
    reader: flate2::read::DeflateDecoder<R>,
}

impl<R: Read> CompressedFileReader<R> {
    pub fn from_compressed(reader: R) -> Result<Self, Ck3Error>
    where
        R: Read,
    {
        let inflater = flate2::read::DeflateDecoder::new(reader);
        Ok(CompressedFileReader { reader: inflater })
    }
}

impl<R> std::io::Read for CompressedFileReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}
