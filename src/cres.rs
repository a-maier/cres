//! Main cell resampling functionality
//!
//! The usual workflow is to construct a [Cres] object from a
//! [CresBuilder].
//!
//! This requires
//! 1. A reader for the input events
//!    (e.g. [hepmc2::Reader](crate::hepmc2::Reader)).
//! 2. A converter to the internal format
//!    (e.g. [hepmc2::ClusteringConverter](crate::hepmc2::ClusteringConverter))
//! 3. A [Resampler](crate::traits::Resample).
//! 4. An [Unweighter](crate::traits::Unweight)
//!    (e.g. [NO_UNWEIGHTING](crate::unweight::NO_UNWEIGHTING)).
//! 5. A [Writer](crate::traits::Write) (e.g. [hepmc2::Writer](crate::hepmc2::Writer)).
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
//!# let reader = cres::hepmc2::Reader::from_filenames(vec![""])?;
//!# let converter = cres::hepmc2::Converter::new();
//!# let resampler = cres::resampler::ResamplerBuilder::default().build();
//!# let writer = cres::hepmc2::WriterBuilder::default().to_filename("")?.build()?;
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

use log::info;
use noisy_float::prelude::*;
use rayon::prelude::*;
use thiserror::Error;

use crate::event::Event;
use crate::traits::*;

/// Build a new [Cres] object
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CresBuilder<R, C, S, U, W> {
    /// Read in events
    pub reader: R,
    /// Convert events into the internal format
    pub converter: C,
    /// Resample events
    pub resampler: S,
    /// Unweight events
    pub unweighter: U,
    /// Write out events
    pub writer: W,
}

impl<R, C, S, U, W> CresBuilder<R, C, S, U, W> {
    /// Construct a [Cres] object
    pub fn build(self) -> Cres<R, C, S, U, W> {
        Cres {
            reader: self.reader,
            converter: self.converter,
            resampler: self.resampler,
            unweighter: self.unweighter,
            writer: self.writer,
        }
    }
}

impl<R, C, S, U, W> From<Cres<R, C, S, U, W>> for CresBuilder<R, C, S, U, W> {
    fn from(b: Cres<R, C, S, U, W>) -> Self {
        CresBuilder {
            reader: b.reader,
            converter: b.converter,
            resampler: b.resampler,
            unweighter: b.unweighter,
            writer: b.writer,
        }
    }
}

/// Main cell resampler
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Cres<R, C, S, U, W> {
    reader: R,
    converter: C,
    resampler: S,
    unweighter: U,
    writer: W,
}

impl<R, C, S, U, W> From<CresBuilder<R, C, S, U, W>> for Cres<R, C, S, U, W> {
    fn from(b: CresBuilder<R, C, S, U, W>) -> Self {
        b.build()
    }
}

#[derive(Debug, Error)]
pub enum CresError<E1, E2, E3, E4, E5, E6> {
    #[error("Failed to read event: {0}")]
    ReadErr(E1),
    #[error("Failed to rewind reader: {0}")]
    RewindErr(E2),
    #[error("Failed to convert event: {0}")]
    ConversionErr(E3),
    #[error("Resampling error: {0}")]
    ResamplingErr(E4),
    #[error("Unweighting error: {0}")]
    UnweightErr(E5),
    #[error("Failed to write events: {0}")]
    WriteErr(E6),
}

impl<R, C, S, U, W, E, Ev> Cres<R, C, S, U, W>
where
    R: Iterator<Item = Result<Ev, E>> + Rewind,
    C: TryConvert<Ev, Event>,
    S: Resample,
    U: Unweight,
    W: Write<R>,
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
            S::Error,
            U::Error,
            W::Error,
        >,
    > {
        use CresError::*;

        self.reader.rewind().map_err(RewindErr)?;

        let converter = &mut self.converter;
        let events: Result<Vec<_>, _> = (&mut self.reader)
            .map(|ev| match ev {
                Ok(ev) => converter.try_convert(ev).map_err(ConversionErr),
                Err(err) => Err(ReadErr(err)),
            })
            .collect();
        let mut events = events?;
        for (id, ev) in events.iter_mut().enumerate() {
            ev.id = id
        }
        info!("Read {} events", events.len());

        let events = self.resampler.resample(events).map_err(ResamplingErr)?;

        let mut events =
            self.unweighter.unweight(events).map_err(UnweightErr)?;
        events.par_sort_unstable();

        let sum_wt: N64 = events.par_iter().map(|e| e.weight).sum();
        let sum_wtsqr: N64 =
            events.par_iter().map(|e| e.weight * e.weight).sum();
        info!("Final sum of weights: {sum_wt:.3e} ± {:.3e}", sum_wtsqr.sqrt());

        self.reader.rewind().map_err(RewindErr)?;
        let reader = &mut self.reader;
        self.writer.write(reader, &events).map_err(WriteErr)
    }
}
