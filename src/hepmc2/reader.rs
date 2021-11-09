use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

use crate::auto_decompress::auto_decompress;
use crate::traits::{TryClone, Rewind};

use hepmc2::reader::{LineParseError, Reader};
use log::info;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError<E>{
    #[error("Error cloning reader for source {1}: {0}")]
    CloneErr(E, usize),
    #[error("Error reading HepMC record from source {1}: {0}")]
    HepMCReadErr(LineParseError, usize),
}

pub struct CombinedReader<'a, R: 'a> {
    next_sources: Vec<R>,
    previous_sources: Vec<R>,
    reader: Reader<Box<dyn BufRead + 'a>>,
}

fn empty_reader() -> Reader<Box<dyn BufRead>> {
    Reader::new(Box::new(BufReader::new(std::io::empty())))
}

impl<'a, R: 'a> CombinedReader<'a, R> {
    pub fn new(sources: Vec<R>) -> Self {
        CombinedReader {
            next_sources: sources,
            previous_sources: Vec::new(),
            reader: empty_reader(),
        }
    }
}

impl CombinedReader<'static, crate::file::File> {
    pub fn from_files(sources: Vec<std::fs::File>) -> Self {
        Self::new(sources.into_iter().map(crate::file::File).collect())
    }
}

impl<'a, R: Seek + 'a> Rewind for CombinedReader<'a, R> {
    type Error = std::io::Error;

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

impl<'a, R: TryClone + Read + 'a> Iterator for CombinedReader<'a, R> {
    type Item = Result<hepmc2::event::Event, ReadError<<R as TryClone>::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        let nsource = self.previous_sources.len();
        if let Some(next) = self.reader.next() {
            debug_assert!(nsource > 0);
            Some(next.map_err(|err| ReadError::HepMCReadErr(err, nsource - 1)))
        } else {
            if let Some(next_source) = self.next_sources.pop() {
                let clone = match next_source.try_clone() {
                    Ok(clone) => clone,
                    Err(err) => return Some(Err(ReadError::CloneErr(err, nsource)))
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
}
