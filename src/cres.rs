use std::convert::From;
use std::iter::Iterator;

use log::info;
use thiserror::Error;

use crate::event::{Event, EventBuilder};
use crate::traits::*;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CresBuilder<R, C, S, U, W> {
    pub reader: R,
    pub converter: C,
    pub resampler: S,
    pub unweighter: U,
    pub writer: W,
}

impl<R, C, S, U, W> CresBuilder<R, C, S, U, W> {
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
    R: Iterator<Item=Result<Ev, E>> + Rewind,
    C: TryConvert<(Ev, EventBuilder), Event>,
    S: Resample,
    U: Unweight,
    W: Write<R>
{

    pub fn run(&mut self) -> Result<(), CresError<E, <R as Rewind>::Error, C::Error, S::Error, U::Error, W::Error>> {
        use CresError::*;

        self.reader.rewind().map_err(
            |err| RewindErr(err)
        )?;

        let converter = &mut self.converter;
        let events: Result<Vec<_>, _> = (&mut self.reader).enumerate().map(
            |(id, ev)| match ev {
                Ok(ev) => {
                    let builder = EventBuilder::new(id);
                    converter.try_convert((ev, builder)).map_err(ConversionErr)
                },
                Err(err) => Err(ReadErr(err))
            }
        ).collect();
        let events = events?;
        info!("Read {} events", events.len());

        let events = self.resampler.resample(events).map_err(
            |err| ResamplingErr(err)
        )?;

        let events = self.unweighter.unweight(events).map_err(
            |err| UnweightErr(err)
        )?;

        self.reader.rewind().map_err(
            |err| RewindErr(err)
        )?;
        let reader = &mut self.reader;
        self.writer.write(reader, &events).map_err(
            |err| WriteErr(err)
        )
    }
}
