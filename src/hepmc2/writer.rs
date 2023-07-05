use std::cell::RefCell;
use std::collections::{hash_map::Entry, HashMap};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::rc::Rc;

use crate::cell_collector::CellCollector;
use crate::compression::{compress_writer, Compression};
use crate::event::Event;
use crate::progress_bar::{Progress, ProgressBar};
use crate::traits::Write;

use derive_builder::Builder;
use thiserror::Error;

/// Write events in HepMC 2 format
#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Writer<T> {
    /// Where to write the events
    writer: T,
    #[builder(default)]
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    /// Output compression
    #[builder(default)]
    compression: Option<Compression>,
}

impl WriterBuilder<BufWriter<File>> {
    /// Write to the file with the given name
    pub fn to_filename<P: AsRef<Path>>(
        self,
        path: P,
    ) -> Result<Self, std::io::Error> {
        let file = File::create(path.as_ref())?;
        Ok(self.writer(BufWriter::new(file)))
    }
}

impl<E, R, T: std::io::Write> Write<R> for Writer<T>
where
    R: Iterator<Item = Result<avery::Event, E>>,
    E: std::error::Error,
{
    type Error = WriteError<E>;

    /// Write all `events`.
    ///
    /// `events` has to be sorted by [id](crate::event::Event::id). The
    /// [Cres](crate::cres::Cres) struct does this automatically.
    ///
    /// For each event `e` in `events`, we read events from `reader`
    /// until the number of read events matches `e.id() + 1`. We then
    /// adjust the weight and cross section of the last read event and
    /// write it out.
    fn write(
        &mut self,
        reader: &mut R,
        events: &[Event],
    ) -> Result<(), Self::Error> {
        use WriteError::*;

        let writer = compress_writer(&mut self.writer, self.compression)?;
        let mut writer = hepmc2::Writer::try_from(writer)?;

        let dump_event_to = self
            .cell_collector
            .clone()
            .map(|c| c.borrow().event_cells());
        let mut cell_writers = HashMap::new();
        for cellnr in
            dump_event_to.iter().flat_map(|c| c.values().flatten())
        {
            if let Entry::Vacant(entry) = cell_writers.entry(cellnr) {
                let file = File::create(format!("cell{}.hepmc", cellnr))?;
                let writer =
                    compress_writer(BufWriter::new(file), self.compression)?;
                let writer = hepmc2::Writer::try_from(writer)?;
                entry.insert(writer);
            }
        }

        let mut reader_events = reader.enumerate();
        let progress = ProgressBar::new(events.len() as u64, "events written:");
        for event in events {
            let (read_id, read_event) = reader_events.next().unwrap();
            let mut read_event = read_event.map_err(ReadErr)?;
            if read_id < event.id() {
                for _ in read_id..event.id() {
                    let (_id, ev) = reader_events.next().unwrap();
                    read_event = ev.map_err(ReadErr)?;
                }
            }
            if read_event.id.is_none() {
                read_event.id = Some(event.id() as i32);
            }
            // TODO: return error
            let weight = read_event.weights.first_mut().unwrap();
            weight.weight = Some(f64::from(event.weight));
            let out_event = read_event.into();
            writer.write(&out_event)?;
            if let Some(dump_event_to) = dump_event_to.as_ref() {
                let cellnums: &[usize] = dump_event_to
                    .get(&event.id())
                    .map(|v: &Vec<usize>| v.as_slice())
                    .unwrap_or_default();
                for cellnum in cellnums {
                    let cell_writer = cell_writers.get_mut(cellnum).unwrap();
                    cell_writer.write(&out_event)?;
                }
            }
            progress.inc(1);
        }
        writer.finish()?;
        for (_, cell_writer) in cell_writers {
            cell_writer.finish()?;
        }
        progress.finish();
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WriteError<E> {
    #[error("Failed to read event: {0}")]
    ReadErr(E),
    #[error("Failed to write event: {0}")]
    WriteErr(#[from] std::io::Error),
}
