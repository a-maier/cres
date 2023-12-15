use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf}, fs::File, string::FromUtf8Error,
};

use audec::auto_decompress;
use log::debug;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{hepmc2::HepMCParser, traits::{Rewind, TryConvert, UpdateWeights}, util::trim_ascii_start, event::{Event, Weights}, progress_bar::{ProgressBar, Progress}, compression::Compression, formats::FileFormat};

#[cfg(feature = "lhef")]
use crate::lhef::LHEFParser;
#[cfg(feature = "ntuple")]
use crate::ntuple::NTupleConverter;
#[cfg(feature = "stripper-xml")]
use crate::stripper_xml::StripperXmlParser;

const ROOT_MAGIC_BYTES: [u8; 4] = [b'r', b'o', b'o', b't'];

/// Event reader from a single file
///
/// The format is determined automatically. If you know the format
/// beforehand, you can use
/// e.g. [hepmc2::FileReader](crate::hepmc2::FileReader) instead.
pub struct FileReader(Box<dyn EventFileReader>);

impl FileReader {
    /// Construct new reader from file
    pub fn try_new(infile: PathBuf) -> Result<Self, CreateError> {
        let format = detect_event_file_format(&infile)?;
        debug!("Read {infile:?} as {format:?} file");
        let reader: Box<dyn EventFileReader> = match format {
            FileFormat::HepMC2 => {
                use crate::hepmc2::FileReader as HepMCReader;
                Box::new(HepMCReader::try_new(infile)?)
            },
            #[cfg(feature = "lhef")]
            FileFormat::Lhef => {
                use crate::lhef::FileReader as LhefReader;
                Box::new(LhefReader::try_new(infile)?)
            },
            #[cfg(feature = "ntuple")]
            FileFormat::BlackHatNtuple => {
                use crate::ntuple::FileReader as NTupleReader;
                Box::new(NTupleReader::try_new(infile)?)
            },
            #[cfg(feature = "stripper-xml")]
            FileFormat::StripperXml => {
                use crate::stripper_xml::FileReader as XMLReader;
                Box::new(XMLReader::try_new(infile)?)
            },
        };
        Ok(Self(reader))
    }
}

impl EventFileReader for FileReader {
    fn path(&self) -> &Path {
        self.0.path()
    }

    fn header(&self) -> &[u8] {
        self.0.header()
    }
}

impl Rewind for FileReader {
    type Error = CreateError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.0.rewind()
    }
}

impl Iterator for FileReader {
    type Item = Result<EventRecord, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// Event storage backed by one input and one output event file
///
/// The format is determined automatically. If you know the format
/// beforehand, you can use
/// e.g. [hepmc2::FileStorage](crate::hepmc2::FileStorage) instead.
pub struct FileStorage(Box<dyn EventFileStorage>);

impl Rewind for FileStorage {
    type Error = FileStorageError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.0.rewind()
    }
}

impl Iterator for FileStorage {
    type Item = Result<EventRecord, FileStorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// Builder for event storages
#[derive(Clone, Debug, Default)]
pub struct StorageBuilder {
    scaling: HashMap<String, f64>,
    compression: Option<Compression>,
    weight_names: Vec<String>,
}

impl StorageBuilder {
    /// Set compression of event output files
    pub fn compression(&mut self, compression: Option<Compression>) -> &mut Self {
        self.compression = compression;
        self
    }

    /// Specify names of weights that should be updated
    pub fn weight_names(&mut self, weight_names: Vec<String> ) -> &mut Self {
        self.weight_names = weight_names;
        self
    }

    /// Build an event storage from the given input and output files
    pub fn build_from_files(
        self,
        infile: PathBuf,
        outfile: PathBuf
    ) -> Result<FileStorage, CreateError> {
        let StorageBuilder { scaling, compression, weight_names } = self;
        let _scaling = scaling;

        let format = detect_event_file_format(&infile)?;
        debug!("Read {infile:?} as {format:?} file");

        let storage: Box<dyn EventFileStorage> = match format {
            FileFormat::HepMC2 => {
                use crate::hepmc2::FileStorage as HepMCStorage;
                Box::new(HepMCStorage::try_new(
                    infile,
                    outfile,
                    compression,
                    weight_names,
                )?)
            },
            #[cfg(feature = "lhef")]
            FileFormat::Lhef => {
                use crate::lhef::FileStorage as LHEFStorage;
                Box::new(LHEFStorage::try_new(
                    infile,
                    outfile,
                    compression,
                    weight_names,
                )?)
            },
            #[cfg(feature = "ntuple")]
            FileFormat::BlackHatNtuple => {
                use crate::ntuple::FileStorage as NTupleStorage;
                Box::new(NTupleStorage::try_new(
                    infile,
                    outfile,
                    weight_names,
                )?)
            },
            #[cfg(feature = "stripper-xml")]
            FileFormat::StripperXml => {
                use crate::stripper_xml::FileStorage as XMLStorage;
                Box::new(XMLStorage::try_new(
                    infile,
                    outfile,
                    compression,
                    weight_names,
                    &_scaling,
                )?)
            },
        };
        Ok(FileStorage(storage))
    }

    /// Construct a new storage backed by the files with the given names
    ///
    /// Each item in `files` should have the form `(sourcefile, sinkfile)`.
    pub fn build_from_files_iter<I, P, Q>(
        self,
        files: I
    ) -> Result<CombinedFileStorage, CombinedBuildError>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        #[cfg(feature = "stripper-xml")]
        {
            let (files, scaling) =
                crate::stripper_xml::extract_scaling(files)?;

            let mut builder = self;
            builder.scaling = scaling;
            Ok(builder.build_from_files_iter_known_scaling(files)?)
        }

        #[cfg(not(feature = "stripper-xml"))]
        Ok(self.build_from_files_iter_known_scaling(files)?)
    }

    fn build_from_files_iter_known_scaling<I, P, Q>(
        self,
        files: I
    ) -> Result<CombinedFileStorage, FileStorageError>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>
    {
        let files = Vec::from_iter(
            files
                .into_iter()
                .map(|(source, sink)| {
                    let infile = source.as_ref().to_path_buf();
                    let outfile = sink.as_ref().to_path_buf();
                    StorageFiles{ infile, outfile }
                })
        );
        CombinedFileStorage::new(files, self)
    }
}

/// Detect format of an event file
///
/// Defaults to [HepMC2] if not other format can be identified.
pub fn detect_event_file_format(infile: &Path) -> Result<FileFormat, CreateError> {
    use CreateError::*;
    use FileFormat::*;

    let file = File::open(infile).map_err(OpenInput)?;
    let mut r = auto_decompress(BufReader::new(file));
    let Ok(bytes) = r.fill_buf() else {
        return Ok(HepMC2)
    };
    if bytes.starts_with(&ROOT_MAGIC_BYTES) {
        #[cfg(not(feature = "ntuple"))]
        return Err(RootUnsupported);
        #[cfg(feature = "ntuple")]
        return Ok(BlackHatNtuple);
    }
    if trim_ascii_start(bytes).starts_with(b"<?xml") {
        #[cfg(not(feature = "stripper-xml"))]
        return Err(XMLUnsupported);
        #[cfg(feature = "stripper-xml")]
        return Ok(StripperXml)
    }
    #[cfg(feature = "lhef")]
    if bytes.starts_with(b"<LesHouchesEvents") {
        return Ok(Lhef)
    }
    Ok(HepMC2)
}

impl UpdateWeights for FileStorage {
    type Error = FileStorageError;

    fn update_all_weights(
        &mut self,
        weights: &[Weights]
    ) -> Result<usize, Self::Error> {
        self.0.update_all_weights(weights)
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights
    ) -> Result<bool, Self::Error> {
        self.0.update_next_weights(weights)
    }

    fn finish_weight_update(&mut self) -> Result<(), Self::Error> {
        self.0.finish_weight_update()
    }
}

/// Error building a combined event storage
#[derive(Debug, Error)]
pub enum CombinedBuildError {
    /// Error building a file-based event storage
    #[error("Failed to build file-based event storage")]
    FileStorage(#[from] FileStorageError),

    #[cfg(feature = "stripper-xml")]
    /// Error extracting weight scaling
    #[error("Failed to extract weight scaling")]
    WeightScaling(#[from] CreateError),
}

/// Error from event storage operations
#[derive(Debug, Error)]
#[error("Error in event storage reading from {infile} and writing to {outfile}")]
pub struct FileStorageError {
    infile: PathBuf,
    outfile: PathBuf,
    source: ErrorKind
}

impl FileStorageError {
    /// New error for storage associated with the given input and output files
    pub fn new(
        infile: PathBuf,
        outfile: PathBuf,
        source: ErrorKind,
    ) -> Self {
        Self {
            infile,
            outfile,
            source,
        }
    }

    /// Path of the file we are reading from
    pub fn infile(&self) -> &PathBuf {
        &self.infile
    }

    /// Path of the file we are writing to
    pub fn outfile(&self) -> &PathBuf {
        &self.outfile
    }
}

/// Error from event storage operations
#[derive(Debug, Error)]
pub enum ErrorKind {
    /// Error creating an event storage
    #[error("Failed to create event storage")]
    Create(#[from] CreateError),
    /// Error reading in or parsing an event record
    #[error("Failed to read event")]
    Read(#[from] ReadError),
    /// Error writing out an event
    #[error("Failed to write event")]
    Write(#[from] WriteError),
}

/// Error creating an event storage
#[derive(Debug, Error)]
pub enum CreateError {
    /// Failed to open input file
    #[error("Failed to open input file")]
    OpenInput(#[source] std::io::Error),
    /// Failed to read from input file
    #[error("Failed to read from input file")]
    Read(#[source] std::io::Error),
    /// Failed to create target file
    #[error("Failed to create target file")]
    CreateTarget(#[source] std::io::Error),
    /// Failed to compress target file
    #[error("Failed to compress target file")]
    CompressTarget(#[source] std::io::Error),
    /// Failed to write to target file
    #[error("Failed to compress target file")]
    Write(#[source] std::io::Error),
    /// UTF8 error
    #[error("UTF8 error")]
    Utf8(#[from] Utf8Error),

    #[cfg(not(feature = "ntuple"))]
    /// Attempt to use unsupported format
    #[error("Support for ROOT ntuple format is not enabled. Reinstall cres with `cargo install cres --features = ntuple`")]
    RootUnsupported,
    #[cfg(not(feature = "stripper-xml"))]
    /// Attempt to use unsupported format
    #[error("Support for STRIPPER XML format is not enabled. Reinstall cres with `cargo install cres --features = stripper-xml`")]
    XMLUnsupported,

    #[cfg(feature = "ntuple")]
    /// ROOT NTuple error
    #[error("{0}")]
    NTuple(String),

    #[cfg(feature = "stripper-xml")]
    /// XML error in STRIPPER XML file
    #[error("XML Error in input file")]
    XMLError(#[from] crate::stripper_xml::Error),
}

/// UTF-8 error
#[derive(Debug, Error)]
pub enum Utf8Error {
    /// UTF8 error
    #[error("UTF8 error")]
    Utf8(#[from] std::str::Utf8Error),
    /// UTF8 error
    #[error("UTF8 error")]
    FromUtf8(#[from] FromUtf8Error),
}

/// Error reading or parsing an event
#[derive(Debug, Error)]
pub enum ReadError {
    /// I/O error
    #[error("I/O error")]
    IO(#[from] std::io::Error),
    /// Failed to find event record entry
    #[error("Failed to find {0} in {1}")]
    FindEntry(&'static str, String),
    /// Missing named weight entry
    #[error("Failed to find weight\"{0}\": Event has weights {1}")]
    FindWeight(String, String),
    /// Invalid entry
    #[error("{value} is not a valid value for {entry} in {record}")]
    InvalidEntry{
        /// Invalid value of the entry
        value: String,
        /// Entry name
        entry: &'static str,
        /// Event record
        record: String,
    },
    /// Failed to parse event record entry
    #[error("Failed to parse {0} in {1}")]
    ParseEntry(&'static str, String),
    /// Entry not recognised
    #[error("Failed to recognise {0} in {1}")]
    UnrecognisedEntry(&'static str, String),
    /// UTF8 error
    #[error("UTF8 error")]
    Utf8(#[from] Utf8Error),

    #[cfg(feature = "ntuple")]
    /// ROOT NTuple error
    #[error("Failed to read NTuple record")]
    NTuple(#[from] ntuple::reader::ReadError),
    #[cfg(feature = "stripper-xml")]
    /// XML error in STRIPPER XML file
    #[error("XML Error in input file")]
    XMLError(#[from] crate::stripper_xml::Error),
}

/// Error writing out an event
#[derive(Debug, Error)]
pub enum WriteError {
    /// I/O error
    #[error("I/O error")]
    IO(#[from] std::io::Error),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct StorageFiles {
    infile: PathBuf,
    outfile: PathBuf,
}

/// Combined storage from several pairs of files
pub struct CombinedFileStorage {
    files: Vec<StorageFiles>,
    current: Option<FileStorage>,
    current_file_idx: usize,
    builder: StorageBuilder,
    nevents_read: usize,
    total_size_hint: (usize, Option<usize>),
}

impl CombinedFileStorage {
    fn new(
        files: Vec<StorageFiles>,
        builder: StorageBuilder,
    ) -> Result<CombinedFileStorage, FileStorageError> {
        let mut res = Self {
            files,
            current: None,
            current_file_idx: 0,
            builder,
            nevents_read: 0,
            total_size_hint: (0, Some(0)),
        };
        res.init()?;
        Ok(res)
    }

    fn open(&mut self, idx: usize) -> Result<(), FileStorageError> {
        let StorageFiles { infile, outfile } = self.files[idx].clone();
        self.current = Some(
            self.builder.clone().build_from_files(infile, outfile).map_err(|source| {
                let StorageFiles { infile, outfile } = self.files[idx].clone();
                FileStorageError{ infile, outfile, source: source.into() }
            })?
        );
        self.current_file_idx = idx;
        Ok(())
    }

    fn init(&mut self) -> Result<(), FileStorageError> {
        if self.files.is_empty() {
            return Ok(());
        }
        for idx in 0..self.files.len() {
            self.open(idx)?;
            self.total_size_hint = combine_size_hints(
                self.total_size_hint,
                self.current.as_ref().unwrap().size_hint()
            );
        }
        self.open(0)?;
        Ok(())
    }
}

fn combine_size_hints(
    mut h: (usize, Option<usize>),
    g: (usize, Option<usize>),
) -> (usize, Option<usize>) {
    h.0 += g.0;
    h.1 = match (h.1, g.1) {
        (None, _) | (_, None) => None,
        (Some(h), Some(g)) => Some(h + g),
    };
    h
}

impl Rewind for CombinedFileStorage {
    type Error = FileStorageError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.current = None;
        self.nevents_read = 0;
        Ok(())
    }
}

impl Iterator for CombinedFileStorage {
    type Item = <FileStorage as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current.as_mut() {
            let next = current.next();
            if next.is_some() {
                self.nevents_read += 1;
                return next;
            }
            if self.current_file_idx + 1 == self.files.len() {
                return None;
            }
            if let Err(err) = self.open(self.current_file_idx + 1) {
                Some(Err(err))
            } else {
                self.next()
            }
        } else if self.files.is_empty() {
            None
        } else {
            if let Err(err) = self.open(0) {
                Some(Err(err))
            } else {
                self.next()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let min = self.total_size_hint.0.saturating_sub(self.nevents_read);
        let max = self.total_size_hint.1
            .map(|max| max.saturating_sub(self.nevents_read));
        (min, max)
    }
}

impl UpdateWeights for CombinedFileStorage {
    type Error = FileStorageError;

    fn update_all_weights(&mut self, weights: &[Weights]) -> Result<usize, Self::Error> {
        self.rewind()?;
        let mut nevent = 0;
        let progress = ProgressBar::new(weights.len() as u64, "events written:");
        for idx in 0..self.files.len() {
            self.open(idx)?;
            let current = self.current.as_mut().unwrap();
            while nevent < weights.len() {
                if !current.update_next_weights(&weights[nevent])? {
                    break;
                }
                progress.inc(1);
                nevent += 1;
            }
            current.finish_weight_update()?;
        }
        progress.finish();
        Ok(nevent)
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights,
    ) -> Result<bool, Self::Error> {
        while self.current_file_idx < self.files.len() {
            let current = self.current.as_mut().unwrap();
            let res = current.update_next_weights(weights)?;
            if res {
                return Ok(true);
            }
            current.finish_weight_update()?;
            self.open(self.current_file_idx + 1)?;
        }
        Ok(false)
    }
}

/// Reader from an event file
pub trait EventFileReader:
    Iterator<Item = Result<EventRecord, ReadError>>
    + Rewind<Error = CreateError>
{
    /// Path to the file we are reading from
    fn path(&self) -> &Path;

    /// Event file header
    fn header(&self) -> &[u8];
}

/// Event storage backed by files
pub trait EventFileStorage:
    Iterator<Item = Result<EventRecord, FileStorageError>>
    + Rewind<Error = FileStorageError>
    + UpdateWeights<Error = FileStorageError>
{
}

impl EventFileStorage for crate::hepmc2::FileStorage {}

#[cfg(feature = "lhef")]
impl EventFileStorage for crate::lhef::FileStorage {}

#[cfg(feature = "ntuple")]
impl EventFileStorage for crate::ntuple::FileStorage {}

#[cfg(feature = "stripper-xml")]
impl EventFileStorage for crate::stripper_xml::FileStorage {}

/// A bare-bones event record
///
/// The intent is to do the minimal amount of non-parallelisable work
/// to extract the necessary information that can later be used to
/// construct [Event] objects in parallel.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum EventRecord {
    /// Bare HepMC event record
    HepMC(String),
    #[cfg(feature = "lhef")]
    /// Bare Les Houches Event Format record
    LHEF(String),
    #[cfg(feature = "ntuple")]
    /// ROOT NTuple event record
    NTuple(Box<ntuple::Event>),
    #[cfg(feature = "stripper-xml")]
    /// STRIPPER XML event record
    StripperXml(String),
}

impl TryFrom<EventRecord> for String {
    type Error = EventRecord;

    fn try_from(e: EventRecord) -> Result<Self, Self::Error> {
        use EventRecord::*;
        match e {
            HepMC(s) => Ok(s),
            #[cfg(feature = "lhef")]
            LHEF(s) => Ok(s),
            #[cfg(feature = "ntuple")]
            ev @ NTuple(_) => Err(ev),
            #[cfg(feature = "stripper-xml")]
            StripperXml(s) => Ok(s),
        }
    }
}

/// Converter from event records to internal event format
#[derive(Deserialize, Serialize)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Converter {
    #[cfg(feature = "multiweight")]
    weight_names: Vec<String>,
}

impl Converter {
    /// Construct new converter
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "multiweight")]
    /// Construct converter including the given weights in the record
    pub fn with_weights(weight_names: Vec<String>) -> Self {
        Self {weight_names}
    }

    /// Access names of weights that should be converted
    #[cfg(feature = "multiweight")]
    pub fn weight_names(&self) -> &[String] {
        self.weight_names.as_ref()
    }
}

impl TryConvert<EventRecord, Event> for Converter {
    type Error = ErrorKind;

    fn try_convert(&self, record: EventRecord) -> Result<Event, Self::Error> {
        let event = match record {
            EventRecord::HepMC(record) => self.parse_hepmc(&record)?,
            #[cfg(feature = "lhef")]
            EventRecord::LHEF(record) => self.parse_lhef(&record)?,
            #[cfg(feature = "ntuple")]
            EventRecord::NTuple(record) => self.convert_ntuple(*record)?,
            #[cfg(feature = "stripper-xml")]
            EventRecord::StripperXml(record) => self.parse_stripper_xml(&record)?,
        };
        Ok(event)
    }
}
