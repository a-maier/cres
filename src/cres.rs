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
use crate::reader::EventRecord;
use crate::traits::*;

/// Build a new [Cres] object
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CresBuilder<R, C, Cl, S, U, W> {
    /// Read in events
    pub reader: R,
    /// Convert events into the internal format
    pub converter: C,
    /// Cluster outgoing particles into IRC safe objects
    pub clustering: Cl,
    /// Resample events
    pub resampler: S,
    /// Unweight events
    pub unweighter: U,
    /// Write out events
    pub writer: W,
}

impl<R, C, Cl, S, U, W> CresBuilder<R, C, Cl, S, U, W> {
    /// Construct a [Cres] object
    pub fn build(self) -> Cres<R, C, Cl, S, U, W> {
        Cres {
            reader: self.reader,
            converter: self.converter,
            clustering: self.clustering,
            resampler: self.resampler,
            unweighter: self.unweighter,
            writer: self.writer,
        }
    }
}

impl<R, C, Cl, S, U, W> From<Cres<R, C, Cl, S, U, W>> for CresBuilder<R, C, Cl, S, U, W> {
    fn from(b: Cres<R, C, Cl, S, U, W>) -> Self {
        CresBuilder {
            reader: b.reader,
            converter: b.converter,
            clustering: b.clustering,
            resampler: b.resampler,
            unweighter: b.unweighter,
            writer: b.writer,
        }
    }
}

/// Main cell resampler
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Cres<R, C, Cl, S, U, W> {
    reader: R,
    converter: C,
    clustering: Cl,
    resampler: S,
    unweighter: U,
    writer: W,
}

impl<R, C, Cl, S, U, W> From<CresBuilder<R, C, Cl, S, U, W>> for Cres<R, C, Cl, S, U, W> {
    fn from(b: CresBuilder<R, C, Cl, S, U, W>) -> Self {
        b.build()
    }
}

/// A cell resampling error
#[derive(Debug, Error)]
pub enum CresError<E1, E2, E3, E4, E5, E6, E7> {
    /// Error reading an event
    #[error("Failed to read event")]
    ReadErr(#[source] E1),
    /// Error rewinding the event reader
    #[error("Failed to rewind reader")]
    RewindErr(#[source] E2),
    /// Error converting event record
    #[error("Failed to convert event record")]
    ConversionErr(#[source] E3),
    /// Error clustering event
    #[error("Failed to cluster event")]
    ClusterErr(#[source] E4),
    /// Error encountered during resampling
    #[error("Resampling error")]
    ResamplingErr(#[source] E5),
    /// Error encountered during unweighting
    #[error("Unweighting error")]
    UnweightErr(#[source] E6),
    /// Error writing resampled events
    #[error("Failed to write events")]
    WriteErr(#[source] E7),
    /// Encountered event with invalid id
    #[error("Encountered event with non-zero id {0}")]
    IdErr(usize),
}

impl<R, C, Cl, S, U, W, E> Cres<R, C, Cl, S, U, W>
where
    R: Iterator<Item = Result<EventRecord, E>> + Rewind,
    C: TryConvert<EventRecord, Event> + Sync,
    Cl: Clustering + Sync,
    S: Resample,
    U: Unweight,
    E: Send,
    <R as Rewind>::Error: Send,
    U::Error: Send,
    S::Error: Send,
    C::Error: Send,
    Cl::Error: Send,
    // W: Write<R>,
{
    /// Run the cell resampler
    ///
    /// This goes through the following steps
    ///
    /// 1. Read in events
    /// 2. Convert events into internal format
    /// 3. Apply cell resampling
    /// 4. Unweight
    /// 5. Write out events
    pub fn run(
        &mut self,
    ) -> Result<
        (),
        CresError<
            E,
            <R as Rewind>::Error,
            C::Error,
            Cl::Error,
            S::Error,
            U::Error,
            std::convert::Infallible
            // W::Error,
        >,
    > {
        use CresError::*;

        self.reader.rewind().map_err(RewindErr)?;

        let expected_nevents = self.reader.size_hint().0;
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
                for (id, record) in (&mut self.reader).enumerate() {
                    let record = record.map_err(ReadErr)?;
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
        info!("Read {} events", events.len());

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

        // self.reader.rewind().map_err(RewindErr)?;
        // let reader = &mut self.reader;
        // self.writer.write(reader, &events).map_err(WriteErr)
        Ok(())
    }
}
