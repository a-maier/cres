use std::{path::{Path, PathBuf}, io::{BufReader, BufRead}};

use hepmc2::reader::LineParseError;
use log::debug;
use thiserror::Error;

use crate::{traits::Rewind, file::File};

const ROOT_MAGIC_BYTES: [u8; 4] = [b'r', b'o', b'o', b't'];

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
    type Item = Result<hepmc2::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// Returns an event reader for the file at `path`
pub fn make_reader<P: AsRef<Path>>(
    path: P
) -> Result<FileReader, CreateError> {
    use crate::hepmc2::FileReader as HepMCReader;
    let file = File::open(&path)?;
    let mut r = BufReader::new(file);
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

    }
    debug!("Read {:?} as HepMC file", path.as_ref());
    let file = File::open(path)?;
    let reader = HepMCReader::new(file)?;
    Ok(FileReader(Box::new(reader)))
}

#[derive(Debug, Error)]
pub enum CreateError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Cannot read ROOT ntuple event file `{0}`. Reinstall cres with `cargo install cres --features = ntuple`")]
    RootUnsupported(PathBuf),
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
    #[error("Error reading HepMC record: {0}")]
    HepMCError(#[from] LineParseError),
    #[cfg(feature = "ntuple")]
    #[error("Error reading ntuple event: {0}")]
    NTupleError(#[from] ::ntuple::reader::ReadError),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CombinedReader<R> {
    readers: Vec<R>,
    current: usize,
}

impl<R> CombinedReader<R> {
    fn new(readers: Vec<R>) -> Self {
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
        let readers: Result<_, _> = files.into_iter().map(make_reader).collect();
        Ok(Self::new(readers?))
    }
}

pub trait EventFileReader:
    Iterator<Item = Result<hepmc2::Event, EventReadError>>
    + Rewind<Error = RewindError> {
    }

#[cfg(feature = "ntuple")]
impl EventFileReader for crate::ntuple::Reader {}

impl EventFileReader for crate::hepmc2::FileReader {}
