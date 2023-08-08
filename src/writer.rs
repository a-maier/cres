use std::{path::{PathBuf, Path}, cell::RefCell, rc::Rc, collections::{HashMap, hash_map::Entry}};
#[cfg(feature = "multiweight")]
use std::collections::HashSet;


use strum::Display;
use thiserror::Error;
use typed_builder::TypedBuilder;

use crate::{traits::{Write, WriteEvent}, event::Event, progress_bar::{ProgressBar, Progress}, compression::Compression, cell_collector::CellCollector};

/// Supported output formats
#[derive(Copy, Clone, Debug, Default, Display, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[strum(serialize_all = "lowercase")]
pub enum OutputFormat {
    /// The HepMC2 format
    ///
    /// Also called `IO_GenEvent`. See the [official HepMC3
    /// website](https://gitlab.cern.ch/hepmc/HepMC3) for details.
    #[default]
    HepMC2,
    /// The [Les Houches Event File](https://arxiv.org/abs/hep-ph/0109068v1) format
    #[cfg(feature = "lhef")]
    Lhef,
    /// The [ROOT ntuple](https://arxiv.org/abs/1310.7439) format
    #[cfg(feature = "ntuple")]
    Root
}

/// General-purpose writer to some event file
#[derive(Debug, TypedBuilder)]
pub struct FileWriter {
    filename: PathBuf,
    #[builder(default)]
    format: OutputFormat,
    #[builder(default)]
    compression: Option<Compression>,
    #[builder(default)]
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    #[cfg(feature = "multiweight")]
    #[builder(default)]
    overwrite_weights: HashSet<String>,
}

impl FileWriter {
    fn write_all<F, R, RE, W>(
        &self,
        mut make_writer: F,
        r: &mut R,
        events: &[Event]
    ) -> Result<(), EventWriteError<RE, std::io::Error>>
    where
        F: FnMut(&Path, Option<Compression>) -> Result<W, std::io::Error>,
        W: WriteEvent<avery::Event, Error = std::io::Error>,
        R: Iterator<Item = Result<avery::Event, RE>>,
        RE: std::error::Error
    {
        use EventWriteError::*;

        let mut writer = make_writer(
            &self.filename,
            self.compression,
        ).map_err(CreateErr)?;

        let dump_event_to = self
            .cell_collector
            .clone()
            .map(|c| c.borrow().event_cells());
        let mut cell_writers = HashMap::new();
        let cellnums = dump_event_to.iter().flat_map(|c| c.values().flatten());
        for cellnum in cellnums {
            if let Entry::Vacant(entry) = cell_writers.entry(cellnum) {
                let filename = format!("cell{cellnum}.{}", self.format);
                let cell_writer = make_writer(
                    filename.as_ref(),
                    self.compression,
                ).map_err(CreateErr)?;
                entry.insert(cell_writer);
            }
        }

        let mut reader_events = r.enumerate();
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
            weight.weight = Some(f64::from(event.weight()));
            #[cfg(feature = "multiweight")]
            {
                let weights = event.weights.lock();
                let mut resampled_weights = weights.iter().skip(1);
                for wt in &mut read_event.weights {
                    if let Some(name) = wt.name.as_ref() {
                        if self.overwrite_weights.contains(name) {
                            wt.weight = Some(f64::from(*resampled_weights.next().unwrap()))
                        }
                    }
                }
            }
            if let Some(dump_event_to) = dump_event_to.as_ref() {
                let cellnums: &[usize] = dump_event_to
                    .get(&event.id())
                    .map(|v: &Vec<usize>| v.as_slice())
                    .unwrap_or_default();
                for cellnum in cellnums {
                    let cell_writer = cell_writers.get_mut(cellnum).unwrap();
                    cell_writer.write(read_event.clone()).map_err(WriteErr)?;
                }
            }
            writer.write(read_event).map_err(WriteErr)?;
            progress.inc(1);
        }
        writer.finish().map_err(WriteErr)?;
        for (_, cell_writer) in cell_writers {
            cell_writer.finish().map_err(WriteErr)?;
        }
        progress.finish();
        Ok(())
    }
}

impl<R, RE> Write<R> for FileWriter
where
    R: Iterator<Item = Result<avery::Event, RE>>,
    RE: std::error::Error,
{
    type Error = EventWriteError<RE, std::io::Error>;

    fn write(
        &mut self,
        r: &mut R,
        events: &[Event]
    ) -> Result<(), Self::Error> {
        use OutputFormat::*;
        match self.format {
            HepMC2 => self.write_all(crate::hepmc2::Writer::try_new, r, events),
            #[cfg(feature = "lhef")]
            Lhef => self.write_all(crate::lhef::Writer::try_new, r, events),
            #[cfg(feature = "ntuple")]
            Root => self.write_all(crate::ntuple::Writer::try_new, r, events),
        }
    }
}

#[derive(Debug, Error)]
pub enum EventWriteError<RE, WE> {
    #[error("Failed to create writer: {0}")]
    CreateErr(std::io::Error),
    #[error("Failed to read event: {0}")]
    ReadErr(RE),
    #[error("Failed to write event: {0}")]
    WriteErr(WE),
}
