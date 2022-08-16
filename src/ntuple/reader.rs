use std::io::{Error, ErrorKind};
use std::fmt::Debug;
use std::path::PathBuf;

use crate::reader::{EventReadError, RewindError};
use crate::traits::Rewind;

#[derive(Debug)]
pub struct Reader {
    r: ntuple::Reader,
    file: PathBuf,
}

impl Reader {
    pub fn new(file: PathBuf) -> Result<Self, Error> {
        if let Some(r) = ntuple::Reader::new(&file) {
            Ok(Self{r , file})
        } else {
            Err(create_error(file))
        }
    }
}

impl Rewind for Reader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.r = ntuple::Reader::new(&self.file).ok_or_else(
            || create_error(&self.file)
        )?;
        Ok(())
    }
}

impl Iterator for Reader {
    type Item = Result<hepmc2::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.r.next() {
            Some(Err(err)) => Some(Err(err.into())),
            Some(Ok(ev)) => Some(Ok((&ev).into())),
            None => None
        }
    }
}

fn create_error(file: impl Debug) -> Error {
    Error::new(
        ErrorKind::Other,
        format!("Failed to create ntuple reader for {file:?}")
    )
}
