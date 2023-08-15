use std::io::ErrorKind;
use std::path::Path;

use crate::compression::Compression;
use crate::traits::WriteEvent;

/// Write events in ROOT ntuple format
#[derive(Debug)]
pub struct Writer(ntuple::Writer);

impl Writer {
    pub fn try_new(
        filename: &Path,
        _: Option<Compression>,
    ) -> Result<Self, std::io::Error> {
        let writer =
            ntuple::Writer::new(filename, "cres ntuple").ok_or_else(|| {
                std::io::Error::new(ErrorKind::Other, "Failed to create writer")
            })?;
        Ok(Self(writer))
    }
}

impl WriteEvent<avery::Event> for Writer {
    type Error = std::io::Error;

    fn write(&mut self, e: avery::Event) -> Result<(), Self::Error> {
        self.0
            .write(&e.into())
            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))
    }
}
