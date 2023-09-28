use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf}, fs::File,
};

use audec::auto_decompress;
use log::debug;
use thiserror::Error;

use crate::{hepmc2::HepMCParser, traits::{Rewind, TryConvert, UpdateWeights}, util::trim_ascii_start, event::{Event, Weights}, progress_bar::{ProgressBar, Progress}, compression::Compression};

const ROOT_MAGIC_BYTES: [u8; 4] = [b'r', b'o', b'o', b't'];

/// Event storage backed by a single event file
///
/// The format is determined automatically. If you know the format
/// beforehand, you can use
/// e.g. [hepmc2::FileStorage](crate::hepmc2::FileStorage) instead.
pub struct FileStorage(Box<dyn EventFileStorage>);

impl Rewind for FileStorage {
    type Error = StorageError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.0.rewind()
    }
}

impl Iterator for FileStorage {
    type Item = Result<EventRecord, StorageError>;

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
        source: PathBuf,
        sink: PathBuf
    ) -> Result<FileStorage, CreateError> {
        let StorageBuilder { scaling, compression, weight_names } = self;
        let _scaling = scaling;

        use crate::hepmc2::FileStorage as HepMCStorage;
        let file = File::open(&source)?;
        let mut r = auto_decompress(BufReader::new(file));
        let bytes = match r.fill_buf() {
            Ok(bytes) => bytes,
            Err(_) => {
                let storage =  HepMCStorage::try_new(
                    source,
                    sink,
                    compression,
                    weight_names,
                )?;
                return Ok(FileStorage(Box::new(storage)))
            }
        };
        if bytes.starts_with(&ROOT_MAGIC_BYTES) {
            if !cfg!(feature = "ntuple") {
                return Err(CreateError::RootUnsupported(source));
            }
            #[cfg(feature = "ntuple")]
            {
                debug!("Read {path:?} as ROOT ntuple");
                todo!();
            }
        } else if trim_ascii_start(bytes).starts_with(b"<?xml") {
            #[cfg(not(feature = "stripper-xml"))]
            return Err(CreateError::XMLUnsupported(source));
            #[cfg(feature = "stripper-xml")]
            {
                debug!("Read {path:?} as STRIPPER XML file");
                todo!();
            }
        }
        #[cfg(feature = "lhef")]
        if bytes.starts_with(b"<LesHouchesEvents") {
            use crate::lhef::FileReader as LHEFReader;
            debug!("Read {path:?} as LHEF file");
            todo!();
        }
        debug!("Read {source:?} as HepMC file");
        let storage = HepMCStorage::try_new(
            source,
            sink,
            compression, // TODO: compression
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
    ) -> Result<CombinedStorage<FileStorage>, CreateError>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        #[cfg(feature = "stripper-xml")]
        {
            let (files, scaling) =
                crate::stripper_xml::reader::extract_scaling(files)?;
            let mut builder = self;
            builder.scaling = scaling;
            builder.build_from_files_iter_known_scaling(files)
        }

        #[cfg(not(feature = "stripper-xml"))]
        return self.build_from_files_iter_known_scaling(files)
    }

    fn build_from_files_iter_known_scaling<I, P, Q>(
        self,
        files: I
    ) -> Result<CombinedStorage<FileStorage>, CreateError>
    where
        I: IntoIterator<Item = (P, Q)>,
        P: AsRef<Path>,
        Q: AsRef<Path>
    {
        let storage: Result<_, _> = files
            .into_iter()
            .map(|(source, sink)| {
                let source = source.as_ref().to_path_buf();
                let sink = sink.as_ref().to_path_buf();
                self.clone().build_from_files(source, sink)
            }).collect();
        Ok(CombinedStorage::new(storage?))
    }
}

impl UpdateWeights for FileStorage {
    type Error = StorageError;

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
}

/// Error creating an event storage
#[derive(Debug, Error)]
pub enum CreateError {
    /// I/O error
    #[error("I/O error")]
    IoError(#[from] std::io::Error),
    /// Error reading from file
    #[error("Failed to read from {0}")]
    FileError(PathBuf, #[source] Box<CreateError>),

    /// Attempt to read from unsupported format
    #[error("Cannot read ROOT ntuple event file `{0}`. Reinstall cres with `cargo install cres --features = ntuple`")]
    RootUnsupported(PathBuf),
    /// Attempt to read from unsupported format
    #[error("Cannot read XML event file `{0}`. Reinstall cres with `cargo install cres --features = stripper-xml`")]
    XMLUnsupported(PathBuf),

    #[cfg(feature = "stripper-xml")]
    /// XML error in STRIPPER XML file
    #[error("XML Error in file `{0}`")]
    XMLError(PathBuf, #[source] crate::stripper_xml::reader::XMLError),
}

/// Error reading an event
#[derive(Debug, Error)]
pub enum StorageError {
    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    /// Error cloning the underlying source
    #[error("Source clone error: {0}")]
    CloneError(std::io::Error),
    /// Error reading an event
    #[error("Error reading HepMC record")]
    HepMCError(#[from] crate::hepmc2::HepMCError),
    #[cfg(feature = "ntuple")]
    /// Error reading a ROOT ntuple event
    #[error("Error reading ntuple event")]
    NTupleError(#[from] ::ntuple::reader::ReadError),
    #[cfg(feature = "stripper-xml")]
    /// Error reading a STRIPPER XML event
    #[error("Error reading STRIPPER XML event")]
    StripperXMLError(#[from] crate::stripper_xml::reader::ReadError),
    #[cfg(feature = "lhef")]
    /// Error reading a LHEF event
    #[error("Error reading LHEF event")]
    LHEFError(#[from] ::lhef::reader::ReadError),
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
    type Error = StorageError;

    fn update_all_weights(&mut self, weights: &[Weights]) -> Result<usize, Self::Error> {
        self.rewind()?;
        let mut nevent = 0;
        let progress = ProgressBar::new(weights.len() as u64, "events written:");
        for source in &mut self.storage {
            while nevent < weights.len() {
                let updated = source.update_next_weights(&weights[nevent])?;
                debug_assert!(updated);
                progress.inc(1);
                nevent += 1;
            }
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
            self.current += 1;
        }
        Ok(false)
    }
}

/// Reader from an event file
pub trait EventFileStorage:
    Iterator<Item = Result<EventRecord, StorageError>>
    + Rewind<Error = StorageError>
    + UpdateWeights<Error = StorageError>
{
}

#[cfg(feature = "ntuple")]
impl EventFileStorage for crate::ntuple::Reader {}

#[cfg(feature = "lhef")]
impl EventFileStorage for crate::lhef::FileReader {}

impl EventFileStorage for crate::hepmc2::FileStorage {}

/// A bare-bones event record
///
/// The intent is to do the minimal amount of non-parallelisable work
/// to extract the necessary information that can later be used to
/// construct [Event] objects in parallel.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum EventRecord {
    /// Bare HepMC event record
    HepMC(String),
}

/// Converter from event records to internal event format
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
    type Error = StorageError;

    fn try_convert(&self, record: EventRecord) -> Result<Event, Self::Error> {
        let event = match record {
            EventRecord::HepMC(record) => self.parse_hepmc(&record)?,
        };
        Ok(event)
    }
}
