use std::{
    collections::HashMap,
    io::{BufReader, BufRead},
    path::{Path, PathBuf},
};

use audec::auto_decompress;
use hepmc2::reader::LineParseError;
use log::debug;
use thiserror::Error;

use crate::{traits::Rewind, file::File, util::trim_ascii_start};

const ROOT_MAGIC_BYTES: [u8; 4] = [b'r', b'o', b'o', b't'];

/// Reader for a single event file
///
/// The format is determined automatically. If you know the format
/// beforehand, you can use
/// e.g. [hepmc2::FileReader](crate::hepmc2::FileReader) instead.
pub struct FileReader (
    Box<dyn EventFileReader>
);

impl Rewind for FileReader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.0.rewind()
    }
}

impl Iterator for FileReader {
    type Item = Result<avery::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl FileReader {
    /// Returns an event reader for the file at `path`
    pub fn new<P: AsRef<Path>>(
        path: P
    ) -> Result<FileReader, CreateError> {
        Self::with_scaling(path, &HashMap::new())
    }

    pub fn with_scaling<P: AsRef<Path>>(
        path: P,
        _scaling: &HashMap<String, f64> // only used in "stripper-xml" feature
    ) -> Result<FileReader, CreateError> {
        use crate::hepmc2::FileReader as HepMCReader;
        let file = File::open(&path)?;
        let mut r = auto_decompress(BufReader::new(file));
        let bytes = match r.fill_buf() {
            Ok(bytes) => bytes,
            Err(_) => {
                let file = File::open(&path)?;
                let reader = HepMCReader::new(file)?;
                return Ok(FileReader(Box::new(reader)))
            },
        };
        if bytes.starts_with(&ROOT_MAGIC_BYTES) {
            let path = path.as_ref().to_owned();
            if !cfg!(feature = "ntuple") {
                return Err(CreateError::RootUnsupported(path));
            }
            #[cfg(feature = "ntuple")]
            {
                debug!("Read {path:?} as ROOT ntuple");
                let reader = crate::ntuple::Reader::new(path)?;
                return Ok(FileReader(Box::new(reader)))
            }
        } else if trim_ascii_start(bytes).starts_with(b"<?xml") {
            let path = path.as_ref();
            #[cfg(not(feature = "stripper-xml"))]
            return Err(CreateError::XMLUnsupported(path.to_owned()));
            #[cfg(feature = "stripper-xml")]
            {
                debug!("Read {path:?} as STRIPPER XML file");
                use crate::stripper_xml::FileReader as XMLReader;
                let file = File::open(path)?;
                let reader = XMLReader::new(file, _scaling)?;
                return Ok(FileReader(Box::new(reader)))
            }
        }
        #[cfg(feature = "lhef")]
        if bytes.starts_with(b"<LesHouchesEvents") {
            use crate::lhef::FileReader as LHEFReader;
            debug!("Read {:?} as LHEF file", path.as_ref());
            let file = File::open(path)?;
            let reader = LHEFReader::new(file)?;
            return Ok(FileReader(Box::new(reader)));
        }
        debug!("Read {:?} as HepMC file", path.as_ref());
        let file = File::open(path)?;
        let reader = HepMCReader::new(file)?;
        Ok(FileReader(Box::new(reader)))
    }
}

#[derive(Debug, Error)]
pub enum CreateError {
    #[error("IO error")]
    IoError(#[from] std::io::Error),
    #[error("Failed to read from {0}")]
    FileError(PathBuf, #[source] Box<CreateError>),

    #[error("Cannot read ROOT ntuple event file `{0}`. Reinstall cres with `cargo install cres --features = ntuple`")]
    RootUnsupported(PathBuf),
    #[error("Cannot read XML event file `{0}`. Reinstall cres with `cargo install cres --features = stripper-xml`")]
    XMLUnsupported(PathBuf),

    #[cfg(feature = "stripper-xml")]
    #[error("XML Error in file `{0}`")]
    XMLError(PathBuf, #[source] crate::stripper_xml::reader::XMLError),
}

#[derive(Debug, Error)]
pub enum RewindError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Source clone error: {0}")]
    CloneError(std::io::Error)
}

#[derive(Debug, Error)]
pub enum EventReadError {
    #[error("Error reading HepMC record")]
    HepMCError(#[from] LineParseError),
    #[cfg(feature = "ntuple")]
    #[error("Error reading ntuple event")]
    NTupleError(#[from] ::ntuple::reader::ReadError),
    #[cfg(feature = "stripper-xml")]
    #[error("Error reading STRIPPER XML event")]
    StripperXMLError(#[from] crate::stripper_xml::reader::ReadError),
    #[cfg(feature = "lhef")]
    #[error("Error reading LHEF event")]
    LHEFError(#[from] ::lhef::reader::ReadError),
}

/// Combined sequential reader from several sources (e.g. files)
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CombinedReader<R> {
    readers: Vec<R>,
    current: usize,
}

impl<R> CombinedReader<R> {
    /// Combine multiple readers into a single one
    pub fn new(readers: Vec<R>) -> Self {
        Self{ readers, current: 0 }
    }
}

impl<R: Rewind> Rewind for CombinedReader<R> {
    type Error = <R as Rewind>::Error;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        for reader in &mut self.readers[..=self.current] {
            reader.rewind()?;
        }
        self.current = 0;
        Ok(())
    }
}

impl<R: Iterator> Iterator for CombinedReader<R> {
    type Item = <R as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.readers[self.current].next();
        if next.is_some() {
            return next;
        }
        if self.current + 1 == self.readers.len() {
            return None;
        }
        self.current += 1;
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.readers[self.current..].iter()
            .map(|r| r.size_hint())
            .reduce(|(accmin, accmax), (min, max)|  {
                let accmax = match (accmax, max) {
                    (Some(accmax), Some(max)) => Some(accmax + max),
                    _ => None
                };
                (accmin + min, accmax)
            }).unwrap_or_default()
    }
}

impl CombinedReader<FileReader> {
    /// Construct a new reader reading from the files with the given names
    pub fn from_files<I, P>(
        files: I
    ) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        #[cfg(feature = "stripper-xml")]
        {
            let (files, scaling) = crate::stripper_xml::reader::extract_scaling(files)?;
            return Self::from_files_with_scaling(files, &scaling)
        }

        #[cfg(not(feature = "stripper-xml"))]
        return Self::from_files_with_scaling(files, &HashMap::new())
    }

    fn from_files_with_scaling<I, P>(
        files: I,
        scaling: &HashMap<String, f64>
    ) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let readers: Result<_, _> = files.into_iter()
            .map(|f| FileReader::with_scaling(f.as_ref(), &scaling).map_err(
                |err| CreateError::FileError(f.as_ref().to_path_buf(), Box::new(err))
            ))
            .collect();
        Ok(Self::new(readers?))
    }
}

pub trait EventFileReader:
    Iterator<Item = Result<avery::Event, EventReadError>>
    + Rewind<Error = RewindError> {
    }

#[cfg(feature = "ntuple")]
impl EventFileReader for crate::ntuple::Reader {}

#[cfg(feature = "lhef")]
impl EventFileReader for crate::lhef::FileReader {}

impl EventFileReader for crate::hepmc2::FileReader {}
