use std::{io::BufWriter, path::Path};

use crate::{
    compression::{compress_writer, Compression},
    file::File,
    traits::WriteEvent, storage::EventRecord,
};

/// Write events in HepMC 2 format
#[derive(Debug)]
pub struct Writer<T: std::io::Write>(T);

impl Writer<Box<dyn std::io::Write>> {
    /// Try to construct a new writer to the file with the given path
    pub fn try_new(
        filename: impl AsRef<Path>,
        compression: Option<Compression>,
    ) -> Result<Self, std::io::Error> {
        let outfile = File::create(filename)?;
        let out = BufWriter::new(outfile);
        let out = compress_writer(out, compression)?;
        Ok(Self(out))
    }
}

impl<T: std::io::Write> WriteEvent<EventRecord> for Writer<T> {
    type Error = std::io::Error;

    fn write(&mut self, e: EventRecord) -> Result<(), Self::Error> {
        let EventRecord::HepMC(record) = e else {
            panic!("Trying to write a non-HepMC record in a HepMC writer")
        };
        self.0.write_all(record.as_bytes())?;
        self.0.write(b"\n")?;
        Ok(())
    }

    fn finish(self) -> Result<(), Self::Error> {
        todo!()
    }
}
