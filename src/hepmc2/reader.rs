use std::io::{BufRead, BufReader, Seek};

use audec::auto_decompress;

use crate::{
    file::File,
    reader::{EventReadError, RewindError},
    traits::{Rewind, TryClone},
};

/// Reader for a single (potentially compressed) HepMC2 event file
pub struct FileReader {
    reader: hepmc2::Reader<Box<dyn BufRead>>,
    source: File,
}

impl FileReader {
    /// Construct a reader for the given (potentially compressed) HepMC2 event file
    pub fn new(source: File) -> Result<Self, std::io::Error> {
        let cloned_source = source.try_clone()?;
        Ok(FileReader {
            source,
            reader: hepmc2::Reader::new(auto_decompress(BufReader::new(
                cloned_source,
            ))),
        })
    }
}

impl Rewind for FileReader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        use RewindError::*;
        self.source.rewind()?;
        let cloned_source = self.source.try_clone().map_err(CloneError)?;
        self.reader =
            hepmc2::Reader::new(auto_decompress(BufReader::new(cloned_source)));

        Ok(())
    }
}

impl Iterator for FileReader {
    type Item = Result<avery::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.next().map(|i| match i {
            Ok(ev) => Ok(ev.into()),
            Err(err) => Err(err.into()),
        })
    }
}
