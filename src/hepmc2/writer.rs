use std::{io::BufWriter, path::Path};

use crate::{traits::WriteEvent, compression::{Compression, compress_writer}, file::File};

/// Write events in HepMC 2 format
#[derive(Debug)]
pub struct Writer<T: std::io::Write> (
    hepmc2::Writer<T>
);

impl Writer<Box<dyn std::io::Write>> {
    pub fn try_new(
        filename: &Path,
        compression: Option<Compression>
    ) -> Result<Self, std::io::Error> {
        let outfile = File::create(filename)?;
        let out = BufWriter::new(outfile);
        let out = compress_writer(out, compression)?;
        let writer = hepmc2::Writer::try_from(out)?;
        Ok(Self(writer))
    }
}

impl<T: std::io::Write> WriteEvent<avery::Event> for Writer<T> {
    type Error = std::io::Error;

    fn write(&mut self, e: avery::Event) -> Result<(), Self::Error> {
        self.0.write(&e.into())
    }

    fn finish(self) -> Result<(), Self::Error> {
        self.0.finish()
    }
}
