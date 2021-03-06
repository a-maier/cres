use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::auto_decompress::auto_decompress;
use crate::traits::{Rewind, TryClone};

use hepmc2::reader::{LineParseError, Reader};
use log::info;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError<E> {
    #[error("Error cloning reader for source {1}: {0}")]
    CloneErr(E, usize),
    #[error("Error reading HepMC record from source {1}: {0}")]
    HepMCReadErr(LineParseError, usize),
}

/// Read events from one or more inputs in HepMC 2 format
pub struct CombinedReader<'a, R: TryClone + Read + Seek + 'a> {
    next_sources: Vec<R>,
    previous_sources: Vec<R>,
    reader: Reader<Box<dyn BufRead + 'a>>,
}

fn empty_reader() -> Reader<Box<dyn BufRead>> {
    Reader::new(Box::new(BufReader::new(std::io::empty())))
}

impl<'a, R: TryClone + Read + Seek + 'a> CombinedReader<'a, R> {
    /// Construct a new reader reading from the given sources
    ///
    /// To read from files, use `from_files` or `from_filenames` instead.
    pub fn new(sources: Vec<R>) -> Self {
        CombinedReader {
            next_sources: sources,
            previous_sources: Vec::new(),
            reader: empty_reader(),
        }
    }
}

impl CombinedReader<'static, crate::file::File> {
    /// Construct a new reader reading from the given files
    pub fn from_files<I>(sources: I) -> Self
    where
        I: IntoIterator<Item = std::fs::File>,
    {
        Self::new(sources.into_iter().map(crate::file::File).collect())
    }

    /// Construct a new reader reading from the files with the given names
    pub fn from_filenames<P, I>(sources: P) -> Result<Self, std::io::Error>
    where
        P: IntoIterator<Item = I>,
        I: AsRef<Path>,
    {
        use crate::file::File;
        let sources: Result<Vec<File>, _> =
            sources.into_iter().map(File::open).collect();
        Ok(Self::new(sources?))
    }
}

impl<'a, R: TryClone + Read + Seek + 'a> Rewind for CombinedReader<'a, R> {
    type Error = std::io::Error;

    /// Try to rewind to the beginning
    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.previous_sources.reverse();
        self.next_sources.append(&mut self.previous_sources);
        for source in &mut self.next_sources {
            source.seek(SeekFrom::Start(0))?;
        }
        self.reader = empty_reader();
        Ok(())
    }
}

impl<'a, R: TryClone + Read + Seek + 'a> Iterator for CombinedReader<'a, R> {
    type Item = Result<hepmc2::event::Event, ReadError<<R as TryClone>::Error>>;

    /// Try to read the next event
    fn next(&mut self) -> Option<Self::Item> {
        let nsource = self.previous_sources.len();
        if let Some(next) = self.reader.next() {
            debug_assert!(nsource > 0);
            Some(next.map_err(|err| ReadError::HepMCReadErr(err, nsource - 1)))
        } else if let Some(next_source) = self.next_sources.pop() {
            let clone = match next_source.try_clone() {
                Ok(clone) => clone,
                Err(err) => {
                    return Some(Err(ReadError::CloneErr(err, nsource)))
                }
            };
            self.previous_sources.push(clone);
            info!(
                "Reading from source {}/{}",
                self.previous_sources.len(),
                self.previous_sources.len() + self.next_sources.len()
            );

            let decoder = auto_decompress(BufReader::new(next_source));
            self.reader = Reader::from(decoder);
            self.next()
        } else {
            None
        }
    }
}
