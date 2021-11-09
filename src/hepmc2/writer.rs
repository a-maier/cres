use std::cell::RefCell;
use std::collections::{HashMap, hash_map::Entry};
use std::fs::File;
use std::io::BufWriter;
use std::rc::Rc;

use crate::compression::{Compression, compress_writer};
use crate::cell_collector::CellCollector;
use crate::event::Event;
use crate::progress_bar::{Progress, ProgressBar};
use crate::traits::Write;

use derive_builder::Builder;
use thiserror::Error;
use log::info;
use noisy_float::prelude::*;
use rayon::prelude::*;

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Writer<T> {
    writer: T,
    #[builder(default = "1.")]
    weight_norm: f64,
    #[builder(default)]
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    #[builder(default)]
    compression: Option<Compression>
}

impl<E, R, T: std::io::Write> Write<R> for Writer<T>
where
    R: Iterator<Item = Result<hepmc2::Event, E>>,
    E: std::error::Error
{
    type Error = WriteError<E>;

    fn write(&mut self, reader: &mut R, events: &[Event]) -> Result<(), Self::Error> {
        use WriteError::*;

        let mut writer = hepmc2::Writer::try_from(&mut self.writer)?;

        let sum_wt: N64 = events.par_iter().map(|e| e.weight).sum();
        let xs = n64(self.weight_norm) * sum_wt;
        let sum_wtsqr: N64 = events.par_iter().map(|e| e.weight * e.weight).sum();
        let xs_err = n64(self.weight_norm) * sum_wtsqr.sqrt();
        info!("Final cross section: σ = {:.3e} ± {:.3e}", xs, xs_err);

        let dump_event_to = self.cell_collector.clone().map(|c| c.borrow().event_cells());
        let mut cell_writers = HashMap::new();
        for cellnr in dump_event_to.iter().map(|c| c.values().flatten()).flatten() {
            if let Entry::Vacant(entry) = cell_writers.entry(cellnr) {
                let file = File::create(format!("cell{}.hepmc", cellnr))?;
                let writer = compress_writer(BufWriter::new(file), self.compression)?;
                let writer = hepmc2::Writer::try_from(writer)?;
                entry.insert(writer);
            }
        }

        let mut hepmc_events = reader.enumerate();
        let progress = ProgressBar::new(events.len() as u64, "events written:");
        for event in events {
            let (hepmc_id, hepmc_event) = hepmc_events.next().unwrap();
            let mut hepmc_event = hepmc_event.map_err(
                |err| ReadErr(err)
            )?;
            if hepmc_id < event.id {
                for _ in hepmc_id..event.id {
                    let (_id, ev) = hepmc_events.next().unwrap();
                    hepmc_event = ev.map_err(
                        |err| ReadErr(err)
                    )?;
                }
            }
            let old_weight = hepmc_event.weights.first().unwrap();
            let reweight: f64 = (event.weight / old_weight).into();
            for weight in &mut hepmc_event.weights {
                *weight *= reweight
            }
            hepmc_event.xs.cross_section = xs.into();
            hepmc_event.xs.cross_section_error = xs_err.into();
            writer.write(&hepmc_event)?;
            if let Some(dump_event_to) = dump_event_to.as_ref() {
                let cellnums: &[usize] = dump_event_to
                    .get(&event.id)
                    .map(|v: &Vec<usize>| v.as_slice())
                    .unwrap_or_default();
                for cellnum in cellnums {
                    let cell_writer = cell_writers.get_mut(cellnum).unwrap();
                    cell_writer.write(&hepmc_event)?;
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