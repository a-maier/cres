use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use audec::auto_decompress;
use log::debug;
use thiserror::Error;

use crate::{file::File, hepmc2::reader::HepMCParser, traits::{Rewind, TryConvert, UpdateWeights}, util::trim_ascii_start, event::{Event, Weights}, progress_bar::{ProgressBar, Progress}};

const ROOT_MAGIC_BYTES: [u8; 4] = [b'r', b'o', b'o', b't'];

/// Event storage backed by a single event file
///
/// The format is determined automatically. If you know the format
/// beforehand, you can use
/// e.g. [hepmc2::FileReader](crate::hepmc2::FileReader) instead.
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
impl FileStorage {
    /// Returns an event storage for the file at `path`
    pub fn new<P: AsRef<Path>>(path: P) -> Result<FileStorage, CreateError> {
        Self::with_scaling(path, &HashMap::new())
    }

    /// Returns an event storage for the file at `path`,
    /// with channel-dependent scaling factors for STRIPPER XML events
    pub fn with_scaling<P: AsRef<Path>>(
        path: P,
        _scaling: &HashMap<String, f64>, // only used in "stripper-xml" feature
    ) -> Result<FileStorage, CreateError> {
        let weight_names = Vec::new(); //TODO
        use crate::hepmc2::FileStorage as HepMCStorage;
        let path = path.as_ref();
        let mut out = path.as_os_str().to_os_string(); // TODO: path
        out.push(".out");
        let out = PathBuf::from(out);
        let file = File::open(&path)?;
        let mut r = auto_decompress(BufReader::new(file));
        let bytes = match r.fill_buf() {
            Ok(bytes) => bytes,
            Err(_) => {
                let storage =  HepMCStorage::try_new(
                    path.to_path_buf(),
                    out,
                    None, // TODO: compression
                    weight_names
                )?;
                return Ok(FileStorage(Box::new(storage)))
            }
        };
        if bytes.starts_with(&ROOT_MAGIC_BYTES) {
            let path = path.to_path_buf();
            if !cfg!(feature = "ntuple") {
                return Err(CreateError::RootUnsupported(path));
            }
            #[cfg(feature = "ntuple")]
            {
                debug!("Read {path:?} as ROOT ntuple");
                let reader = crate::ntuple::Reader::new(path)?;
                return Ok(FileStorage(Box::new(reader)));
            }
        } else if trim_ascii_start(bytes).starts_with(b"<?xml") {
            #[cfg(not(feature = "stripper-xml"))]
            return Err(CreateError::XMLUnsupported(path.to_owned()));
            #[cfg(feature = "stripper-xml")]
            {
                debug!("Read {path:?} as STRIPPER XML file");
                use crate::stripper_xml::FileReader as XMLReader;
                let file = File::open(path)?;
                let reader = XMLReader::new(file, _scaling)?;
                return Ok(FileStorage(Box::new(reader)));
            }
        }
        #[cfg(feature = "lhef")]
        if bytes.starts_with(b"<LesHouchesEvents") {
            use crate::lhef::FileReader as LHEFReader;
            debug!("Read {path:?} as LHEF file");
            let file = File::open(path)?;
            let reader = LHEFReader::new(file)?;
            return Ok(FileStorage(Box::new(reader)));
        }
        debug!("Read {path:?} as HepMC file");
        let storage = HepMCStorage::try_new(
            path.to_path_buf(),
            out,
            None, // TODO: compression
            weight_names
        )?;
        Ok(FileStorage(Box::new(storage)))
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
    weight_names: Vec<String>,
}

impl<R> CombinedStorage<R> {
    /// Combine multiple event storages into a single one
    pub fn new(storage: Vec<R>) -> Self {
        Self {
            storage,
            current: 0,
            weight_names: Vec::new(),
        }
    }

    pub fn weight_names(&self) -> &[String] {
        self.weight_names.as_ref()
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

impl CombinedStorage<FileStorage> {
    /// Construct a new storage backed by the files with the given names
    pub fn from_files<I, P>(files: I) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        #[cfg(feature = "stripper-xml")]
        {
            let (files, scaling) =
                crate::stripper_xml::reader::extract_scaling(files)?;
            Self::from_files_with_scaling(files, &scaling)
        }

        #[cfg(not(feature = "stripper-xml"))]
        return Self::from_files_with_scaling(files, &HashMap::new());
    }

    fn from_files_with_scaling<I, P>(
        files: I,
        scaling: &HashMap<String, f64>,
    ) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let storage: Result<_, _> = files
            .into_iter()
            .map(|f| {
                FileStorage::with_scaling(f.as_ref(), scaling).map_err(|err| {
                    CreateError::FileError(
                        f.as_ref().to_path_buf(),
                        Box::new(err),
                    )
                })
            })
            .collect();
        Ok(Self::new(storage?))
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
