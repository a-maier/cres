//! Main cell resampling functionality
//!
//! The usual workflow is to construct a [Cres] object from a
//! [CresBuilder].
//!
//! This requires
//! 1. An object for event input and output
//!    (e.g. [CombinedFileIO](crate::io::CombinedFileIO)).
//! 2. A converter to the internal format
//!    (e.g. [Converter](crate::io::Converter))
//! 3. A clustering of outgoing particles into IRC safe objects
//!    (e.g. [DefaultClustering](crate::cluster::DefaultClustering))
//! 4. A [Resampler](crate::traits::Resample).
//! 5. An [Unweighter](crate::traits::Unweight)
//!    (e.g. [NO_UNWEIGHTING](crate::unweight::NO_UNWEIGHTING)).
//!
//! Finally, call [Cres::run].
//!
//! # Example
//!
//! ```no_run
//!# fn cres_doc() -> Result<(), Box<dyn std::error::Error>> {
//! use cres::prelude::*;
//!
//! // Define `event_io`, `converter`, `clustering`, `resampler`, `unweighter`
//!# let filename = std::path::PathBuf::from("");
//!# let event_io = IOBuilder::default().build_from_files(filename.clone(), filename)?;
//!# let converter = Converter::new();
//!# let clustering = NO_CLUSTERING;
//!# let resampler = cres::resampler::ResamplerBuilder::default().build();
//!# let unweighter = cres::unweight::NO_UNWEIGHTING;
//!
//! // Build the resampler
//! let mut cres = CresBuilder {
//!     event_io,
//!     converter,
//!     clustering,
//!     resampler,
//!     unweighter,
//! }.build();
//!
//! // Run the resampler
//! let result = cres.run();
//!# Ok(())
//!# }
//! ```
//!
use std::collections::HashMap;
use std::convert::From;
use std::iter::Iterator;

use itertools::Itertools;
use log::{info, log_enabled, warn};
use noisy_float::prelude::*;
use parking_lot::Mutex;
use particle_id::ParticleID;
use rayon::prelude::*;
use thiserror::Error;

use crate::event::Event;
use crate::io::EventRecord;
use crate::progress_bar::ProgressBar;
use crate::traits::*;

/// Build a new [Cres] object
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CresBuilder<R, C, Cl, S, U> {
    /// External event I/O, e.g. backed by an event file
    pub event_io: R,
    /// Convert events into the internal format
    pub converter: C,
    /// Cluster outgoing particles into IRC safe objects
    pub clustering: Cl,
    /// Resample events
    pub resampler: S,
    /// Unweight events
    pub unweighter: U,
}

impl<R, C, Cl, S, U> CresBuilder<R, C, Cl, S, U> {
    /// Construct a [Cres] object
    pub fn build(self) -> Cres<R, C, Cl, S, U> {
        Cres {
            event_io: self.event_io,
            converter: self.converter,
            clustering: self.clustering,
            resampler: self.resampler,
            unweighter: self.unweighter,
        }
    }
}

impl<R, C, Cl, S, U> From<Cres<R, C, Cl, S, U>>
    for CresBuilder<R, C, Cl, S, U>
{
    fn from(b: Cres<R, C, Cl, S, U>) -> Self {
        CresBuilder {
            event_io: b.event_io,
            converter: b.converter,
            clustering: b.clustering,
            resampler: b.resampler,
            unweighter: b.unweighter,
        }
    }
}

/// Main cell resampler
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Cres<R, C, Cl, S, U> {
    event_io: R,
    converter: C,
    clustering: Cl,
    resampler: S,
    unweighter: U,
}

impl<R, C, Cl, S, U> From<CresBuilder<R, C, Cl, S, U>>
    for Cres<R, C, Cl, S, U>
{
    fn from(b: CresBuilder<R, C, Cl, S, U>) -> Self {
        b.build()
    }
}

/// A cell resampling error
#[derive(Debug, Error)]
pub enum CresError<E1, E2, E3, E4, E5> {
    /// Error in event I/O
    #[error("Event I/O error")]
    IOErr(#[source] E1),
    /// Error converting event record
    #[error("Failed to convert event record")]
    ConversionErr(#[source] E2),
    /// Error clustering event
    #[error("Failed to cluster event")]
    ClusterErr(#[source] E3),
    /// Error encountered during resampling
    #[error("Resampling error")]
    ResamplingErr(#[source] E4),
    /// Error encountered during unweighting
    #[error("Unweighting error")]
    UnweightErr(#[source] E5),
    /// Encountered event with invalid id
    #[error("Encountered event with non-zero id {0}")]
    IdErr(usize),
}

/// Alias for a cell resampling error
pub type CellResError<R, C, Cl, S, U> = CresError<
    <R as UpdateWeights>::Error,
    <C as TryConvert<EventRecord, Event>>::Error,
    <Cl as Clustering>::Error,
    <S as Resample>::Error,
    <U as Unweight>::Error,
>;

impl<R, C, Cl, S, U> Cres<R, C, Cl, S, U>
where
    R: UpdateWeights,
    R: Iterator<Item = Result<EventRecord, <R as UpdateWeights>::Error>>,
    C: TryConvert<EventRecord, Event> + Sync,
    Cl: Clustering + Sync,
    S: Resample,
    U: Unweight,
    // TODO: logically only C::Error and Cl::Error have to be Send
    C::Error: Send,
    Cl::Error: Send,
    U::Error: Send,
    S::Error: Send,
    <R as UpdateWeights>::Error: Send,
{
    /// Run the cell resampler
    ///
    /// This goes through the following steps
    ///
    /// 1. Read in events
    /// 2. Convert events into internal format
    /// 3. Apply cell resampling
    /// 4. Unweight
    /// 5. Update event weights, rereading the original records
    pub fn run(&mut self) -> Result<(), CellResError<R, C, Cl, S, U>> {
        use CresError::*;

        let mut events = self.read_events()?;
        let nevents = events.len();
        info!("Read {nevents} events");

        events.retain(|e| !e.outgoing().is_empty());
        if events.len() < nevents {
            warn!(
                "Ignoring {} events without identified particles",
                nevents - events.len()
            );
        }
        log_multiplicities(&events);

        self.resampler.resample(&events).map_err(ResamplingErr)?;

        self.unweighter.unweight(&events).map_err(UnweightErr)?;
        events.par_sort_unstable();

        let sum_wt: N64 = events.par_iter().map(|e| e.weight()).sum();
        let sum_neg_wt: N64 = events
            .par_iter()
            .map(|e| e.weight())
            .filter(|&w| w < 0.)
            .sum();
        let sum_wtsqr: N64 =
            events.par_iter().map(|e| e.weight() * e.weight()).sum();
        info!(
            "Final sum of weights: {sum_wt:.3e} ± {:.3e}",
            sum_wtsqr.sqrt()
        );
        info!(
            "Final negative weight fraction: {:.3}",
            -sum_neg_wt / (sum_wt - sum_neg_wt * 2.)
        );

        let mut weights = vec![crate::event::Weights::default(); nevents];
        for event in events {
            weights[event.id] = event.weights.into_inner();
        }

        self.event_io.update_all_weights(&weights).map_err(IOErr)?;
        Ok(())
    }

    fn read_events(
        &mut self,
    ) -> Result<Vec<Event>, CellResError<R, C, Cl, S, U>> {
        use CresError::*;

        let expected_nevents = self.event_io.size_hint().0;
        let event_progress = if expected_nevents > 0 {
            ProgressBar::new(expected_nevents as u64, "events read")
        } else {
            info!("Reading events");
            ProgressBar::default()
        };
        let events = Mutex::new(Vec::with_capacity(expected_nevents));
        {
            let converter = &self.converter;
            let clustering = &self.clustering;
            let events = &events;
            let progress = &event_progress;
            rayon::in_place_scope_fifo(|s| {
                for (id, record) in (&mut self.event_io).enumerate() {
                    let record = record.map_err(IOErr)?;
                    match record {
                        #[cfg(feature = "ntuple")]
                        EventRecord::NTuple(_) => {
                            // sequential conversion is faster than
                            // parallel for this format
                            let ev = converter
                                .try_convert(record)
                                .map_err(ConversionErr)?;
                            let mut ev =
                                clustering.cluster(ev).map_err(ClusterErr)?;
                            if ev.id != 0 {
                                return Err(IdErr(ev.id));
                            }
                            ev.id = id;
                            events.lock().push(Ok(ev));
                            progress.inc(1)
                        }
                        _ => s.spawn_fifo(move |_| {
                            let ev = match converter.try_convert(record) {
                                Ok(ev) => match clustering.cluster(ev) {
                                    Ok(mut ev) => {
                                        if ev.id != 0 {
                                            Err(IdErr(ev.id))
                                        } else {
                                            ev.id = id;
                                            Ok(ev)
                                        }
                                    }
                                    Err(err) => Err(ClusterErr(err)),
                                },
                                Err(err) => Err(ConversionErr(err)),
                            };
                            events.lock().push(ev);
                            progress.inc(1)
                        }),
                    }
                }
                Ok(())
            })?;
        }
        event_progress.finish();
        events.into_inner().into_iter().collect()
    }
}

fn log_multiplicities(events: &[Event]) {
    const MAX_MULT_SHOWN: usize = 1000;
    if log_enabled!(log::Level::Warn) {
        let mut multiplicities: HashMap<_, usize> = HashMap::new();
        for event in events {
            let out_multiplicities = Vec::from_iter(
                event.outgoing().iter().map(|(id, p)| (*id, p.len())),
            );
            *multiplicities.entry(out_multiplicities).or_default() += 1;
        }
        let mut multiplicities = Vec::from_iter(multiplicities);
        multiplicities.sort_unstable_by(|a, b| (b.1, &a.0).cmp(&(a.1, &b.0)));
        for (types, nevents) in multiplicities.iter().take(MAX_MULT_SHOWN) {
            if types.is_empty() {
                info!("{nevents} events without identified particles");
            } else {
                info!(
                    "{nevents} events with {}",
                    types
                        .iter()
                        .map(|(t, n)| format!("{n} {}", name(*t)))
                        .join(", ")
                );
            }
        }
        if multiplicities.len() > MAX_MULT_SHOWN {
            warn!(
                "Found more than {MAX_MULT_SHOWN} event categories ({})",
                multiplicities.len()
            );
        }
    }
}

fn name(t: ParticleID) -> String {
    use crate::cluster;
    t.name()
        .map(|n| format!("{n}s"))
        .unwrap_or_else(|| match t {
            cluster::PID_JET => "jets".to_string(),
            cluster::PID_DRESSED_LEPTON => "dressed leptons".to_string(),
            cluster::PID_ISOLATED_PHOTON => "isolated photons".to_string(),
            _ => format!("particles with id {}", t.id()),
        })
}
