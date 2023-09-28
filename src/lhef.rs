use std::{path::{PathBuf, Path}, io::{BufRead, Write, BufWriter, BufReader}, fs::File};

use audec::auto_decompress;
use log::trace;
use noisy_float::prelude::*;
use nom::{sequence::preceded, character::complete::{i32, space0, u32}, IResult, multi::count};
use particle_id::ParticleID;
use thiserror::Error;

use crate::{compression::{Compression, compress_writer}, traits::{Rewind, UpdateWeights}, storage::{StorageError, EventRecord, Converter}, event::{Weights, Event, EventBuilder}, parsing::{any_entry, double_entry, i32_entry, non_space}, hepmc2::update_central_weight};

/// Storage backed by (potentially compressed) Les Houches Event Files
pub struct FileStorage {
    source_path: PathBuf,
    source: Box<dyn BufRead>,
    _sink_path: PathBuf,
    sink: Box<dyn Write>,
    _weight_names: Vec<String>,
}

impl FileStorage {
    /// Construct a storage backed by the given (potentially compressed) Les Houches Event Files
    pub fn try_new( // TODO: use builder pattern instead?
        source_path: PathBuf,
        sink_path: PathBuf,
        compression: Option<Compression>,
        _weight_names: Vec<String>
    ) -> Result<Self, std::io::Error> {
        let (header, source) = init_source(&source_path)?;
        let outfile = File::create(&sink_path)?;
        let sink = BufWriter::new(outfile);
        let mut sink = compress_writer(sink, compression)?;
        sink.write_all(&header)?;

        Ok(FileStorage {
            source_path,
            source,
            _sink_path: sink_path,
            sink,
            _weight_names,
        })
    }

    fn read_record(&mut self) -> Option<Result<String, Error>> {
        let mut record = Vec::new();
        while !record.ends_with(b"</event>") {
            match self.source.read_until(b'>', &mut record) {
                Ok(0) => return None, // TODO: check incomplete record
                Ok(_) => {},
                Err(err) => return Some(Err(err.into())),
            }
        }
        let record = String::from_utf8(record).unwrap();
        trace!("Read Les Houches Event record:\n{record}");
        Some(Ok(record))
    }
}

impl Rewind for FileStorage {
    type Error = StorageError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        (_, self.source) = init_source(&self.source_path)?;

        Ok(())
    }
}

impl Iterator for FileStorage {
    type Item = Result<EventRecord, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.read_record()
            .map(|r| match r {
                Ok(record) => Ok(EventRecord::LHEF(record)),
                Err(err) => Err(err.into()),
            })
    }
}

fn init_source(source: impl AsRef<Path>) -> Result<(Vec<u8>, Box<dyn BufRead>), std::io::Error> {
    let source = File::open(source)?;
    let mut buf = auto_decompress(BufReader::new(source));

    // read until start of first event
    let mut header = Vec::new();
    while !header.ends_with(b"\n</init>") {
        if buf.read_until(b'>', &mut header)? == 0 {
            break;
        }
    }
    Ok((header, buf))
}

/// Parser for Les Houches Event Format records
pub trait LHEFParser {
    /// Error parsing Les Houches Event Format record
    type Error;

    /// Parse Les Houches Event Format event record
    fn parse_lhef(&self, record: &str) -> Result<Event, Self::Error>;
}

impl LHEFParser for Converter {
    type Error = Error;

    fn parse_lhef(&self, mut record: &str) -> Result<Event, Self::Error> {
        const STATUS_OUTGOING: i32 = 1;

        // TODO: multiple weights
        record = record.trim_start();
        let Some(line_end) = record.find('\n') else {
            return Err(Error::NoLineBreak(record.to_string()));
        };
        record = &record[(1 + line_end)..];
        let (rest, nparticles) = preceded(space0, u32)(record)?;
        let nparticles = nparticles as usize;
        let mut event = EventBuilder::with_capacity(nparticles - 2);
        let (rest, _idrup) = any_entry(rest)?;
        let (rest, wt) = double_entry(rest)?;
        event.add_weight(n64(wt));
        for line in rest.lines().skip(1).take(nparticles) {
            let (rest, id) = preceded(space0, i32)(line)?;
            let id = ParticleID::new(id);
            let (rest, status) = i32_entry(rest)?;
            if status != STATUS_OUTGOING {
                continue;
            }
            // ignore decay parents & colour
            let (rest, _) = count(any_entry, 4)(rest)?;
            let (rest, px) = double_entry(rest)?;
            let (rest, py) = double_entry(rest)?;
            let (rest, pz) = double_entry(rest)?;
            let (_, e) = double_entry(rest)?;
            event.add_outgoing(id, [n64(e), n64(px), n64(py), n64(pz)].into());
        }

        Ok(event.build())
    }
}

impl UpdateWeights for FileStorage {
    type Error = StorageError;

    fn update_all_weights(
        &mut self,
        weights: &[Weights]
    ) -> Result<usize, Self::Error> {
        self.rewind()?;
        for (n, weight) in weights.iter().enumerate() {
            if !self.update_next_weights(weight)? {
                return Ok(n)
            }
        }
        Ok(weights.len())
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights
    ) -> Result<bool, Self::Error> {
        let Some(record) = self.read_record() else {
            return Ok(false)
        };
        let mut record = record?;

        let (rest, _non_weight) = non_weight_entries(&record)
            .map_err(Error::from)?;

        let weights_start = record.len() - rest.len();
        update_central_weight(&mut record, weights_start, weights)?;

        #[cfg(feature = "multiweight")]
        if weights.len() > 1 {
            unimplemented!("Multiple weights in LHEF")
        }
        self.sink.write_all(record.as_bytes())?;
        Ok(true)
    }
}

/// Les Houches Event Format error
#[derive(Debug, Error)]
pub enum Error {
    /// No line breaks
    #[error("No line breaks in event record {0}")]
    NoLineBreak(String),
    /// Parse error
    #[error("Error parsing entry in event record: {0}")]
    ParseError(String),
    /// I/O error
    #[error("I/O error")]
    IOError(#[from] std::io::Error),
}

impl From<nom::Err<nom::error::Error<&str>>> for Error {
    fn from(source: nom::Err<nom::error::Error<&str>>) -> Self {
        Self::ParseError(source.to_string())
    }
}

fn non_weight_entries(record: &str) -> IResult<&str, &str> {
    let record = record.trim_start();
    let line_end = record.find('\n').unwrap(); // TODO: error treatment
    let rest = &record[(line_end + 1)..];
    let (rest, _nup) = preceded(space0, non_space)(rest)?;
    let (rest, _idrup) = any_entry(rest)?;
    let (parsed, rest) = record.split_at(record.len() - rest.len());
    Ok((rest, parsed))
}
