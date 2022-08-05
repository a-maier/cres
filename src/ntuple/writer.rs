use std::{path::Path, io::BufWriter};

use crate::{traits::Write, file::File};

/// Write internal events in HepMC 2 format
pub struct Writer<W> {
    writer: W,
}

impl Writer<BufWriter<File>> {
    /// Write to the file with the given name
    pub fn to_filename<P: AsRef<Path>>(
        self,
        path: P,
    ) -> Result<Self, std::io::Error> {
        let file = File::create(path.as_ref())?;
        Ok(Writer{ writer: BufWriter::new(file) })
    }
}


impl<R, W> Write<R> for Writer<W> {
    type Error = std::convert::Infallible;

    fn write(
        &mut self,
        _r: &mut R,
        _e: &[crate::event::Event]
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
