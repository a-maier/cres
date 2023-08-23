use std::{io::BufWriter, path::Path};

use log::error;
use stripper_xml::{Eventrecord, SubEvent, WriteXML};

use crate::{
    compression::{compress_writer, Compression},
    file::File,
    traits::WriteEvent,
    VERSION,
};

/// Write events in STRIPPER XML format
#[derive(Debug)]
pub struct Writer<T: std::io::Write> {
    out: T,
    finished: bool,
    record: Eventrecord,
}

impl<T: std::io::Write> Writer<T> {
    fn ref_finish(&mut self) -> Result<(), std::io::Error> {
        self.finished = true;
        self.write_record()
    }

    fn write_record(&mut self) -> Result<(), std::io::Error> {
        let mut record = std::mem::take(&mut self.record);
        if record.events.is_empty() {
            return Ok(());
        }
        record.nevents = record.events.len() as u64;
        record.nsubevents = record.nevents;
        record.nreweights = record
            .events
            .iter()
            .flat_map(|ev| ev.subevents.iter())
            .map(|ev| ev.reweight.len() as u64)
            .sum();
        record.write(&mut self.out)
    }
}

impl Writer<Box<dyn std::io::Write>> {
    /// Try to construct a writer to the file with the given path
    pub fn try_new(
        filename: &Path,
        compression: Option<Compression>,
    ) -> Result<Self, std::io::Error> {
        let outfile = File::create(filename)?;
        let out = BufWriter::new(outfile);
        let mut out = compress_writer(out, compression)?;
        out.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
"#,
        )?;
        out.write_all(
            format!("<!-- File generated with cres {VERSION} -->\n").as_bytes(),
        )?;
        Ok(Self {
            out,
            finished: false,
            record: Default::default(),
        })
    }
}

impl<T: std::io::Write> WriteEvent<avery::Event> for Writer<T> {
    type Error = std::io::Error;

    fn write(&mut self, mut e: avery::Event) -> Result<(), Self::Error> {
        if e.info != self.record.name {
            self.write_record()?;
            self.record.name = std::mem::take(&mut e.info);
        }
        let scale = e
            .attr
            .get("wtscale")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let alpha_s_power = e.attr.get("as").and_then(|s| s.parse().ok());
        if let Some(alpha_s_power) = alpha_s_power {
            self.record.alpha_s_power = alpha_s_power;
        }

        let mut subev = SubEvent::from(e);
        subev.weight /= scale;
        let ev = stripper_xml::Event {
            subevents: vec![subev],
        };
        self.record.events.push(ev);
        Ok(())
    }

    fn finish(mut self) -> Result<(), Self::Error> {
        self.ref_finish()
    }
}

impl<T: std::io::Write> Drop for Writer<T> {
    fn drop(&mut self) {
        if !self.finished {
            error!("STRIPPER XML writer dropped before finished.");
            error!("Call finish() manually to fix this error.");
            if let Err(err) = self.ref_finish() {
                error!("Error writing last record: {}", err);
            }
        }
    }
}
