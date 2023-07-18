use std::io::{Error, ErrorKind};
use std::fmt::Debug;
use std::path::PathBuf;

use crate::reader::{EventReadError, RewindError};
use crate::traits::Rewind;

/// Reader for a single ROOT ntuple event file
#[derive(Debug)]
pub struct Reader (
    ntuple::Reader,
);

impl Reader {
    /// Construct a reader for the ROOT ntuple file with the given name
    pub fn new(file: PathBuf) -> Result<Self, Error> {
        let r = ntuple::Reader::new(&file).ok_or_else(
            || create_error(file)
        )?;
        Ok(Self(r))
    }
}

impl Rewind for Reader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        *self.0.nevent_mut() = 0;
        Ok(())
    }
}

impl Iterator for Reader {
    type Item = Result<avery::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Err(err)) => Some(Err(err.into())),
            Some(Ok(ev)) => Some(Ok(ev.into())),
            None => None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

fn create_error(file: impl Debug) -> Error {
    Error::new(
        ErrorKind::Other,
        format!("Failed to create ntuple reader for {file:?}")
    )
}
