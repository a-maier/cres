use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf}, fs::File, string::FromUtf8Error,
};

use audec::auto_decompress;
use log::debug;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{hepmc2::HepMCParser, traits::{Rewind, TryConvert, UpdateWeights}, util::trim_ascii_start, event::{Event, Weights}, progress_bar::{ProgressBar, Progress}, compression::Compression};

#[cfg(feature = "lhef")]
use crate::lhef::LHEFParser;
#[cfg(feature = "ntuple")]
use crate::ntuple::NTupleConverter;
#[cfg(feature = "stripper-xml")]
use crate::stripper_xml::StripperXmlParser;

const ROOT_MAGIC_BYTES: [u8; 4] = [b'r', b'o', b'o', b't'];

/// Event storage backed by a single event file
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
        use CreateError::*;

        let StorageBuilder { scaling, compression, weight_names } = self;
        let _scaling = scaling;

        use crate::hepmc2::FileStorage as HepMCStorage;
        let file = File::open(&infile).map_err(OpenInput)?;
        let mut r = auto_decompress(BufReader::new(file));
        let bytes = match r.fill_buf() {
            Ok(bytes) => bytes,
            Err(_) => {
                let storage = HepMCStorage::try_new(
                    infile,
                    outfile,
                    compression,
                    weight_names,
                )?;
                return Ok(FileStorage(Box::new(storage)))
            }
        };
        if bytes.starts_with(&ROOT_MAGIC_BYTES) {
            #[cfg(not(feature = "ntuple"))]
            return Err(RootUnsupported);
            #[cfg(feature = "ntuple")]
            {
                use crate::ntuple::FileStorage as NTupleStorage;
                debug!("Read {infile:?} as ROOT ntuple");
                let storage = NTupleStorage::try_new(
                    infile,
                    outfile,
                    weight_names,
                )?;
                return Ok(FileStorage(Box::new(storage)))
            }
        } else if trim_ascii_start(bytes).starts_with(b"<?xml") {
            #[cfg(not(feature = "stripper-xml"))]
            return Err(XMLUnsupported);
            #[cfg(feature = "stripper-xml")]
            {
                use crate::stripper_xml::FileStorage as XMLStorage;
                debug!("Read {infile:?} as ROOT ntuple");
                let storage = XMLStorage::try_new(
                    infile,
                    outfile,
                    compression,
                    weight_names,
                    &_scaling,
                )?;
                return Ok(FileStorage(Box::new(storage)))
            }
        }
        #[cfg(feature = "lhef")]
        if bytes.starts_with(b"<LesHouchesEvents") {
            use crate::lhef::FileStorage as LHEFStorage;
            debug!("Read {infile:?} as LHEF file");
            let storage =  LHEFStorage::try_new(
                infile,
                outfile,
                compression,
                weight_names,
            )?;
            return Ok(FileStorage(Box::new(storage)))
        }
        debug!("Read {infile:?} as HepMC file");
        let storage = HepMCStorage::try_new(
            infile,
            outfile,
            compression,
            weight_names
        )?;
        Ok(FileStorage(Box::new(storage)))
    }

    /// Construct a new storage backed by the files with the given names
    ///
    /// Each item in `files` should have the form `(sourcefile, sinkfile)`.
    pub fn build_from_files_iter<I, P, Q>(
        self,
        files: I
    ) -> Result<CombinedStorage<FileStorage>, CombinedBuildError>
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
    ) -> Result<CombinedStorage<FileStorage>, FileStorageError>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>
    {
        let storage: Result<_, _> = files
            .into_iter()
            .map(|(source, sink)| {
                let infile = source.as_ref().to_path_buf();
                let outfile = sink.as_ref().to_path_buf();
                self.clone().build_from_files(infile.clone(), outfile.clone())
                    .map_err(|err| FileStorageError { infile, outfile, source: err.into() })
            }).collect();
        Ok(CombinedStorage::new(storage?))
    }
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

/// Combined storage from several sources (e.g. files)
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CombinedStorage<R> {
    storage: Vec<R>,
    current: usize,
}

impl<R> CombinedStorage<R> {
    /// Combine multiple event storages into a single one
    pub fn new(storage: Vec<R>) -> Self {
        Self {
            storage,
            current: 0,
        }
    }
}

impl<R: Rewind> Rewind for CombinedStorage<R> {
    type Error = <R as Rewind>::Error;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        for storage in &mut self.storage[..=self.current] {
            storage.rewind()?;
        }
        self.current = 0;
        Ok(())
    }
}

impl<R: Iterator> Iterator for CombinedStorage<R> {
    type Item = <R as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.storage[self.current].next();
        if next.is_some() {
            return next;
        }
        if self.current + 1 == self.storage.len() {
            return None;
        }
        self.current += 1;
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.storage[self.current..]
            .iter()
            .map(|r| r.size_hint())
            .reduce(|(accmin, accmax), (min, max)| {
                let accmax = match (accmax, max) {
                    (Some(accmax), Some(max)) => Some(accmax + max),
                    _ => None,
                };
                (accmin + min, accmax)
            })
            .unwrap_or_default()
    }
}

impl UpdateWeights for CombinedStorage<FileStorage> {
    type Error = FileStorageError;

    fn update_all_weights(&mut self, weights: &[Weights]) -> Result<usize, Self::Error> {
        self.rewind()?;
        let mut nevent = 0;
        let progress = ProgressBar::new(weights.len() as u64, "events written:");
        for source in &mut self.storage {
            while nevent < weights.len() {
                if !source.update_next_weights(&weights[nevent])? {
                    break;
                }
                progress.inc(1);
                nevent += 1;
            }
            source.finish_weight_update()?;
        }
        progress.finish();
        Ok(nevent)
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights,
    ) -> Result<bool, Self::Error> {
        while self.current < self.storage.len() {
            let res = self.storage[self.current].update_next_weights(weights)?;
            if res {
                return Ok(true);
            }
            self.storage[self.current].finish_weight_update()?;
            self.current += 1;
        }
        Ok(false)
    }
}

/// Reader from an event file
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
