use std::io::{Seek, BufRead, BufReader};


use crate::{traits::{TryClone, Rewind}, reader::{RewindError, EventReadError}, file::File, auto_decompress::auto_decompress};

pub struct FileReader {
    reader: hepmc2::Reader<Box<dyn BufRead>>,
    source: File,
}

impl FileReader {
    pub fn new(source: File) -> Result<Self, std::io::Error> {
        let cloned_source = source.try_clone()?;
        Ok(FileReader {
            source,
            reader: hepmc2::Reader::new(auto_decompress(BufReader::new(cloned_source)))
        })
    }
}

impl Rewind for FileReader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        use RewindError::*;
        self.source.rewind()?;
        let cloned_source = self.source.try_clone().map_err(CloneError)?;
        self.reader = hepmc2::Reader::new(auto_decompress(BufReader::new(cloned_source)));

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
