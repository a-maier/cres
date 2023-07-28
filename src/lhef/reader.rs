use std::io::{Error, ErrorKind, BufReader, BufRead, Seek};
use std::fmt::{Debug, Display};

use crate::auto_decompress::auto_decompress;
use crate::file::File;
use crate::reader::{RewindError, EventReadError};
use crate::traits::{Rewind, TryClone};

/// Read events in [Les Houches Event File](https://arxiv.org/abs/hep-ph/0109068v1) format from a (potentially compressed) file
pub struct FileReader {
    reader: ::lhef::Reader<Box<dyn BufRead>>,
    source: File,
}

impl FileReader {
    pub fn new(source: File) -> Result<Self, std::io::Error> {
        let cloned_source = source.try_clone()?;
        let input = auto_decompress(BufReader::new(cloned_source));
        let reader = ::lhef::Reader::new(input).map_err(
            |err| create_error(&source, err)
        )?;
        Ok(FileReader {
            source,
            reader
        })
    }
}

impl Rewind for FileReader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        use RewindError::*;
        self.source.rewind()?;
        let cloned_source = self.source.try_clone().map_err(CloneError)?;
        let input = auto_decompress(BufReader::new(cloned_source));
        self.reader = ::lhef::Reader::new(input).map_err(
            |err| create_error(&self.source, err)
        )?;

        Ok(())
    }
}

impl Iterator for FileReader {
    type Item = Result<avery::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.hepeup()
            .transpose()
            .map(|r| match r{
                Ok(hepeup) => Ok((self.reader.heprup().to_owned(), hepeup).into()),
                Err(err) => Err(err.into()),
            })
    }
}

fn create_error(
    file: impl Debug,
    err: impl Display
) -> Error {
    Error::new(
        ErrorKind::Other,
        format!("Failed to create LHEF reader for {file:?}: {err}")
    )
}
