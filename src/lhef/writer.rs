use std::{io::{BufWriter, ErrorKind}, path::Path};

use crate::{traits::WriteEvent, compression::{Compression, compress_writer}, file::File, GIT_REV, GIT_BRANCH, VERSION};

/// Write events in the [Les Houches Event File](https://arxiv.org/abs/hep-ph/0109068v1) format
#[derive(Debug)]
pub struct Writer<T: std::io::Write> (
    lhef::Writer<T>
);

impl Writer<Box<dyn std::io::Write>> {
    pub fn try_new(
        filename: &Path,
        compression: Option<Compression>
    ) -> Result<Self, std::io::Error> {
        let outfile = File::create(filename)?;
        let out = BufWriter::new(outfile);
        let out = compress_writer(out, compression)?;
        let writer = lhef::Writer::new(out, "1.0").map_err(
            |e| std::io::Error::new(ErrorKind::Other, e)
        )?;
        Ok(Self(writer))
    }
}

impl<T: std::io::Write> WriteEvent<avery::Event> for Writer<T> {
    type Error = std::io::Error;

    fn write(&mut self, e: avery::Event) -> Result<(), Self::Error> {
        use lhef::writer::WriterState::ExpectingHeaderOrInit;

        let hepeup;
        if self.0.state() == ExpectingHeaderOrInit {
            let (heprup, ev) = e.into();
            let header = if let (Some(rev), Some(branch)) = (GIT_REV, GIT_BRANCH) {
                format!("generated with cres {} rev {rev} ({branch})", VERSION)
            } else {
                format!("generated with cres {}", VERSION)
            };
            self.0.header(&header).map_err(
                |e| std::io::Error::new(ErrorKind::Other, e)
            )?;
            self.0.heprup(&heprup).map_err(
                |e| std::io::Error::new(ErrorKind::Other, e)
            )?;
            hepeup = ev;
        } else {
            hepeup = e.into();
        }
        self.0.hepeup(&hepeup).map_err(
            |e| std::io::Error::new(ErrorKind::Other, e)
        )
    }

    fn finish(mut self) -> Result<(), Self::Error> {
        self.0.finish().map_err(|e| std::io::Error::new(ErrorKind::Other, e))
    }
}
