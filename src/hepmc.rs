use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use crate::auto_decompress::auto_decompress;
use crate::traits::Rewind;

use hepmc2::reader::{LineParseError, Reader};
use log::info;

pub struct CombinedReader {
    next_files: Vec<File>,
    previous_files: Vec<File>,
    reader: Reader<Box<dyn BufRead>>,
}

pub type HepMCReader = CombinedReader;

fn empty_reader() -> Reader<Box<dyn BufRead>> {
    Reader::new(Box::new(BufReader::new(std::io::empty())))
}

impl CombinedReader {
    pub fn new(files: Vec<File>) -> Self {
        CombinedReader {
            next_files: files,
            previous_files: Vec::new(),
            reader: empty_reader(),
        }
    }

}

impl Rewind for CombinedReader {
    type Error = std::io::Error;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.previous_files.reverse();
        self.next_files.append(&mut self.previous_files);
        for file in &mut self.next_files {
            file.seek(SeekFrom::Start(0))?;
        }
        self.reader = empty_reader();
        Ok(())
    }
}

impl Iterator for CombinedReader {
    type Item = Result<hepmc2::event::Event, LineParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.reader.next();
        if next.is_none() {
            if let Some(next_file) = self.next_files.pop() {
                self.previous_files.push(next_file.try_clone().unwrap());
                info!(
                    "Reading from file {}/{}",
                    self.previous_files.len(),
                    self.previous_files.len() + self.next_files.len()
                );

                let decoder = auto_decompress(BufReader::new(next_file));
                self.reader = Reader::from(decoder);
                self.next()
            } else {
                None
            }
        } else {
            next
        }
    }
}
