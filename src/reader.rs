use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    io::{BufReader, BufRead}
};

use hepmc2::reader::LineParseError;
use log::debug;
use noisy_float::prelude::*;
use thiserror::Error;

use crate::{traits::Rewind, file::File, auto_decompress::auto_decompress};

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

impl FileReader {
    /// Returns an event reader for the file at `path`
    pub fn new<P: AsRef<Path>>(
        path: P
    ) -> Result<FileReader, CreateError> {
        Self::with_scaling(path, &HashMap::new())
    }

    pub fn with_scaling<P: AsRef<Path>>(
        path: P,
        _scaling: &HashMap<String, N64> // only used in "stripper-xml" feature
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
            let path = path.as_ref().to_owned();
            if !cfg!(feature = "stripper-xml") {
                return Err(CreateError::XMLUnsupported(path));
            }
            #[cfg(feature = "stripper-xml")]
            {
                debug!("Read {path:?} as STRIPPER XML file");
                let reader = crate::stripper_xml::Reader::new(path, _scaling)?;
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
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to read from {0}")]
    FileError(PathBuf, #[source] Box<CreateError>),

    #[error("Cannot read ROOT ntuple event file `{0}`. Reinstall cres with `cargo install cres --features = ntuple`")]
    RootUnsupported(PathBuf),

    #[error("Cannot read XML event file `{0}`. Reinstall cres with `cargo install cres --features = stripper-xml`")]
    XMLUnsupported(PathBuf),

    #[cfg(feature = "stripper-xml")]
    #[error("Failed to read XML file `{0}`: {1}")]
    XmlError(PathBuf, crate::stripper_xml::XMLError),
    #[cfg(feature = "stripper-xml")]
    #[error("Missing normalization for part `{0}`")]
    NoScale(String),
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
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Error reading HepMC record: {0}")]
    HepMCError(#[from] LineParseError),
    #[cfg(feature = "lhef")]
    #[error("Error reading LHEF event: {0}")]
    LHEFError(#[from] ::lhef::reader::ReadError),
    #[cfg(feature = "stripper-xml")]
    #[error("Error reading STRIPPER XML event: {0}")]
    XMLError(#[from] quick_xml::DeError),
    #[cfg(feature = "ntuple")]
    #[error("Error reading ntuple event: {0}")]
    NTupleError(#[from] ::ntuple::reader::ReadError),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Reader<R> {
    readers: Vec<R>,
    current: usize,
}

impl<R> Reader<R> {
    fn new(readers: Vec<R>) -> Self {
        Self{ readers, current: 0 }
    }
}

impl<R: Rewind> Rewind for Reader<R> {
    type Error = <R as Rewind>::Error;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        for reader in &mut self.readers[..=self.current] {
            reader.rewind()?;
        }
        self.current = 0;
        Ok(())
    }
}

impl<R: Iterator> Iterator for Reader<R> {
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

impl Reader<FileReader> {
    /// Construct a new reader reading from the files with the given names
    pub fn from_files<I, P>(
        files: I
    ) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        #[cfg(feature = "stripper-xml")]
        return Self::from_files_with_stripper_xml(files);

        #[cfg(not(feature = "stripper-xml"))]
        return Self::from_event_files(files, &HashMap::new());
    }

    #[cfg(feature = "stripper-xml")]
    fn from_files_with_stripper_xml<I, P>(
        paths: I
    ) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        use crate::stripper_xml::{extract_xml_info, XMLTag};

        let paths = paths.into_iter();
        let mut event_files = Vec::with_capacity(paths.size_hint().0);
        let mut rescale: HashMap<_, (N64, u64)> = HashMap::new();
        for path in paths {
            let file = File::open(&path)?;
            let mut r = auto_decompress(BufReader::new(file));
            match r.fill_buf() {
                Ok(buf) => {
                    let buf = trim_ascii_start(buf);
                    if buf.starts_with(b"<?xml") {
                        let tag = extract_xml_info(path.as_ref(), buf).map_err(
                            |err| CreateError::XmlError(path.as_ref().to_owned(), err)
                        )?;
                        match tag {
                            XMLTag::Normalization { name, scale } => {
                                let mut entry = rescale.entry(name).or_default();
                                entry.0 = scale;
                                // don't need the file anymore
                            },
                            XMLTag::Eventrecord { name, nevents, .. } => {
                                let mut entry = rescale.entry(name)
                                    .or_insert((n64(-1.), 0));
                                entry.1 += nevents;
                                event_files.push(path);
                            },
                        }
                    } else {
                        // not a STRIPPER XML file
                        event_files.push(path);
                    }
                },
                _ => event_files.push(path)
            }
        }
        let rescale: HashMap<_, _> = rescale.into_iter()
            .map(|(name, (scale, nevents))| (name, scale / n64(nevents as f64)))
            .collect();
        for (part, &scale) in &rescale {
            if scale < 0. {
                return Err(CreateError::NoScale(part.to_string()));
            }
        }
        debug!("Channel rescaling factors: {rescale:#?}");
        Self::from_event_files(event_files, &rescale)
    }

    fn from_event_files<I, P>(
        files: I,
        channel_scaling: &HashMap<String, N64>
    ) -> Result<Self, CreateError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let readers: Result<_, _> = files.into_iter()
            .map(|f| {
                FileReader::with_scaling(
                    f.as_ref(),
                    channel_scaling
                ).map_err(
                    |err| CreateError::FileError(
                        f.as_ref().to_path_buf(),
                        Box::new(err)
                    )
                )
            }).collect();
        Ok(Self::new(readers?))
    }
}

pub trait EventFileReader:
    Iterator<Item = Result<hepmc2::Event, EventReadError>>
    + Rewind<Error = RewindError> {
    }

#[cfg(feature = "ntuple")]
impl EventFileReader for crate::ntuple::Reader {}

#[cfg(feature = "stripper-xml")]
impl EventFileReader for crate::stripper_xml::Reader {}

#[cfg(feature = "lhef")]
impl EventFileReader for crate::lhef::FileReader {}

impl EventFileReader for crate::hepmc2::FileReader {}

// the corresponding built-in method is currently (rust 1.70.0) only
// available in unstable, so we have to implement it ourselves
fn trim_ascii_start(buf: &[u8]) -> &[u8] {
    if let Some(pos) = buf.iter().position(|b| ! b.is_ascii_whitespace()) {
        &buf[pos..]
    } else {
        buf
    }
}
