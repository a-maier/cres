//! Main cell resampling functionality
//!
//! The usual workflow is to construct a [Cres] object from a
//! [CresBuilder].
//!
//! This requires
//! 1. A reader for the input events
//!    (e.g. [CombinedReader](crate::reader::CombinedReader)).
//! 2. A converter to the internal format
//!    (e.g. [ClusteringConverter](crate::converter::ClusteringConverter))
//! 3. A [Resampler](crate::traits::Resample).
//! 4. An [Unweighter](crate::traits::Unweight)
//!    (e.g. [NO_UNWEIGHTING](crate::unweight::NO_UNWEIGHTING)).
//! 5. A [Writer](crate::traits::Write) (e.g. [FileWriter](crate::writer::FileWriter)).
//!
//! Finally, call [Cres::run].
//!
//! # Example
//!
//! ```no_run
//!# fn cres_doc() -> Result<(), Box<dyn std::error::Error>> {
//! use cres::prelude::*;
//!
//! // Define `reader`, `converter`, `resampler`, `unweighter`, `writer`
//!# let reader = CombinedReader::from_files(vec![""])?;
//!# let converter = cres::converter::Converter::new();
//!# let resampler = cres::resampler::ResamplerBuilder::default().build();
//!# let writer = cres::writer::FileWriter::builder().filename("out.hepmc".into()).build();
//!# let unweighter = cres::unweight::NO_UNWEIGHTING;
//!
//! // Build the resampler
//! let mut cres = CresBuilder {
//!     reader,
//!     converter,
//!     resampler,
//!     unweighter,
//!     writer
//! }.build();
//!
//! // Run the resampler
//! let result = cres.run();
//!# Ok(())
//!# }
//! ```
//!
use std::convert::From;
use std::iter::Iterator;

use log::{info, trace};
use noisy_float::prelude::*;
use parking_lot::Mutex;
use rayon::prelude::*;
use thiserror::Error;

use crate::event::Event;
use crate::progress_bar::ProgressBar;
use crate::storage::EventRecord;
use crate::traits::*;

/// Build a new [Cres] object
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CresBuilder<R, C, Cl, S, U> {
    /// External event storage, e.g. backed by an event file
    pub event_storage: R,
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
            event_storage: self.event_storage,
            converter: self.converter,
            clustering: self.clustering,
            resampler: self.resampler,
            unweighter: self.unweighter,
        }
    }
}

impl<R, C, Cl, S, U> From<Cres<R, C, Cl, S, U>> for CresBuilder<R, C, Cl, S, U> {
    fn from(b: Cres<R, C, Cl, S, U>) -> Self {
        CresBuilder {
            event_storage: b.event_storage,
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
    event_storage: R,
    converter: C,
    clustering: Cl,
    resampler: S,
    unweighter: U,
}

impl<R, C, Cl, S, U> From<CresBuilder<R, C, Cl, S, U>> for Cres<R, C, Cl, S, U> {
    fn from(b: CresBuilder<R, C, Cl, S, U>) -> Self {
        b.build()
    }
}

/// A cell resampling error
#[derive(Debug, Error)]
pub enum CresError<E1, E2, E3, E4, E5> {
    /// Error accessing event storage
    #[error("Event storage error")]
    StorageErr(#[source] E1),
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
    /// 1. Read in events from storage
    /// 2. Convert events into internal format
    /// 3. Apply cell resampling
    /// 4. Unweight
    /// 5. Update event weights in storage
    pub fn run(
        &mut self,
    ) -> Result<
        (),
        CresError<
            <R as UpdateWeights>::Error,
            C::Error,
            Cl::Error,
            S::Error,
            U::Error,
        >,
    > {
        use CresError::*;

        let expected_nevents = self.event_storage.size_hint().0;
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
                for (id, record) in (&mut self.event_storage).enumerate() {
                    let record = record.map_err(StorageErr)?;
                    s.spawn_fifo(move |_| {
                        let ev = match converter.try_convert(record) {
                            Ok(ev) => match clustering.cluster(ev) {
                                Ok(mut ev) => if ev.id != 0 {
                                    Err(IdErr(ev.id))
                                } else {
                                    ev.id = id;
                                    Ok(ev)
                                }
                                Err(err) => Err(ClusterErr(err)),
                            }
                            Err(err) => Err(ConversionErr(err)),
                        };
                        events.lock().push(ev);
                        progress.inc(1)
                    });
                }
                Ok(())
            })?;
        }
        event_progress.finish();
        let events: Result<Vec<_>, _> = events.into_inner().into_iter().collect();
        let events = events?;
        let nevents = events.len();
        info!("Read {nevents} events");

        let events = self.resampler.resample(events).map_err(ResamplingErr)?;

        let mut events =
            self.unweighter.unweight(events).map_err(UnweightErr)?;
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

        self.event_storage.update_all_weights(&weights).map_err(StorageErr)?;
        Ok(())
    }
}
