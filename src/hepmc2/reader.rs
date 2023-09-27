use std::io::{BufRead, BufReader, Seek};

use audec::auto_decompress;
use log::trace;
use noisy_float::prelude::*;
use nom::{multi::count, character::complete::{char, i32, space1, u32}, sequence::{preceded, delimited}, number::complete::double, IResult, bytes::complete::{take_until, take_while1}};
use particle_id::ParticleID;
use thiserror::Error;

use crate::{
    file::File,
    storage::{EventReadError, RewindError, EventRecord, Converter},
    traits::{Rewind, TryClone}, event::{Event, EventBuilder},
};

/// Reader for a single (potentially compressed) HepMC2 event file
pub struct FileReader {
    buf: Box<dyn BufRead>,
    source: File,
}

impl FileReader {
    /// Construct a reader for the given (potentially compressed) HepMC2 event file
    pub fn new(source: File) -> Result<Self, std::io::Error> {
        let cloned_source = source.try_clone()?;
        let mut buf = init_buf(cloned_source)?;
        Ok(FileReader {
            source,
            buf,
        })
    }
}

fn init_buf(source: File) -> Result<Box<dyn BufRead>, std::io::Error> {
    let mut buf = auto_decompress(BufReader::new(source));

    // read until start of first event
    let mut dump = Vec::new();
    while !dump.ends_with(b"\nE") {
        dump.clear();
        if buf.read_until(b'E', &mut dump)? == 0 {
            break;
        }
    }
    Ok(buf)
}

impl Rewind for FileReader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        use RewindError::*;
        self.source.rewind()?;
        let cloned_source = self.source.try_clone().map_err(CloneError)?;
        self.buf = init_buf(cloned_source)?;

        Ok(())
    }
}

impl Iterator for FileReader {
    type Item = Result<EventRecord, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut record = vec![b'E'];
        assert!(record.starts_with(b"E"));
        while !record.ends_with(b"\nE") {
            assert!(record.starts_with(b"E"));
            match self.buf.read_until(b'E', &mut record) {
                Ok(0) => if record.len() > 1 {
                    break;
                } else {
                    return None;
                },
                Ok(_) => {},
                Err(err) => return Some(Err(HepMCError::from(err).into())),
            }
        }
        record.truncate(record.len() - 2);
        let record = String::from_utf8(record).unwrap();
        assert!(record.starts_with("E"));
        trace!("Read HepMC record:\n{record}");
        Some(Ok(EventRecord::HepMC(record)))
    }
}

/// Error reading a HepMC event record
#[derive(Debug, Error)]
pub enum HepMCError {
    /// Parse error
    #[error("Error parsing line in event record: {0}")]
    ParseError(String),
    /// Invalid start of record
    #[error("Record does not start with 'E': {0}")]
    BadRecordStart(String),
    /// Unrecognized entry
    #[error("Line does not correspond to a known entry type: {0}")]
    BadEntry(String),
    /// I/O error
    #[error("I/O error")]
    IOError(#[from] std::io::Error),
    /// Invalid energy unit
    #[error("Invalid energy unit: {0}")]
    InvalidEnergyUnit(String),
    /// Weight not found
    #[error("Failed to find weight\"{0}\": Event has weights {1}")]
    WeightNotFound(String, String),
}

pub trait HepMCParser {
    type Error;

    fn parse_hepmc(&self, record: &str) -> Result<Event, Self::Error>;
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum EnergyUnit {
    MeV,
    #[default]
    GeV,
}

impl HepMCParser for Converter {
    type Error = HepMCError;

    fn parse_hepmc(&self, mut record: &str) -> Result<Event, Self::Error> {
        let mut event = EventBuilder::new();
        let (weight, _weights, rest) = extract_weights(record)?;
        event.add_weight(n64(weight));
        record = rest;

        #[allow(non_snake_case)]
        let mut energy_unit = EnergyUnit::GeV;
        while let Some(pos) = record.find('\n') {
            record = &record[(pos + 1)..];
            match record.as_bytes().first() {
                Some(b'V') | Some(b'F') | Some(b'H') | Some(b'C') => { }
                #[cfg(not(feature = "multiweight"))]
                Some(b'N') => { }
                #[cfg(feature = "multiweight")]
                Some(b'N') => record = parse_weight_names_line(
                    &self,
                    record,
                    &_weights,
                    &mut event
                )?,
                Some(b'P') => record = parse_particle_line(&record, &mut event)?,
                Some(b'U') => (energy_unit, record) = parse_units_line(&record)?,
                _ => if !record.trim().is_empty() {
                    return Err(HepMCError::BadEntry(record.to_owned()));
                }
            }
        }

        if energy_unit != EnergyUnit::GeV {
            assert_eq!(energy_unit, EnergyUnit::MeV);
            event.rescale_energies(n64(1e-3));
        }
        Ok(event.build())
    }
}

fn parse_units_line(record: &str) -> Result<(EnergyUnit, &str), HepMCError> {
    debug_assert!(record.starts_with('U'));
    let (rest, energy) = any_entry(&record[1..])?;
    let energy = match energy {
        "GEV" => EnergyUnit::GeV,
        "MEV" => EnergyUnit::MeV,
        _ => return Err(HepMCError::InvalidEnergyUnit(energy.to_owned()))
    };
    Ok((energy, rest))
}

fn parse_particle_line<'a>(record: &'a str, event: &mut EventBuilder) -> Result<&'a str, HepMCError> {
    const HEPMC_OUTGOING: i32 = 1;

    debug_assert!(record.starts_with('P'));
    let (rest, _barcode) = any_entry(&record[1..])?;
    let (rest, id) = i32_entry(rest)?;
    let (rest, px) = double_entry(rest)?;
    let (rest, py) = double_entry(rest)?;
    let (rest, pz) = double_entry(rest)?;
    let (rest, e) = double_entry(rest)?;
    let (rest, _m) = any_entry(rest)?;
    let (rest, status) = i32_entry(rest)?;
    if status != HEPMC_OUTGOING {
        return Ok(rest);
    }
    event.add_outgoing(ParticleID::new(id), [n64(e), n64(px), n64(py), n64(pz)].into());
    Ok(rest)
}


fn extract_weights(record: &str) -> Result<(f64, Vec<f64>, &str), HepMCError> {
    if !record.starts_with('E') {
        return Err(HepMCError::BadRecordStart(record.to_owned()));
    }
    // ignore first 10 entries
    let (rest, _) = count(any_entry, 10)(&record[1..])?;
    let (rest, nrandom_states) = u32_entry(rest)?;
    // ignore random states
    let (rest, _) = count(any_entry, nrandom_states as usize)(rest)?;
    let (rest, nweights) = u32_entry(rest)?;
    let res = if cfg!(feature = "multiweight") {
        let (rest, weights) = count(double_entry, nweights as usize)(rest)?;
        (weights[0], weights, rest)
    } else {
        let (rest, weight) = double_entry(rest)?;
        (weight, vec![], rest)
    };
    Ok(res)
}

#[cfg(feature = "multiweight")]
fn parse_weight_names_line<'a>(
    converter: &Converter,
    mut record: &'a str,
    all_weights: &[f64],
    event: &mut EventBuilder,
) -> Result<&'a str, HepMCError> {
    use std::collections::HashMap;

    let weight_names = converter.weight_names();
    if weight_names.is_empty() {
        return Ok(record);
    }
    let mut weight_seen: HashMap<_, _> = weight_names.iter()
        .map(|n| (n.as_str(), false))
        .collect();
    let (names, nnames) = u32_entry(&record[1..])?;
    record = names;
    for i in 0..(nnames as usize) {
        let name;
        (record, name) = string_entry(record)?;
        if let Some(seen) = weight_seen.get_mut(name) {
            *seen = true;
            event.add_weight(n64(all_weights[i]));
        }
    }
    let missing =
        weight_seen
        .into_iter()
        .find_map(|(name, seen)| if seen { None } else { Some(name) });
    if let Some(missing) = missing {
        Err(HepMCError::WeightNotFound(
            missing.to_owned(),
            names.to_owned(),
        ))
    } else {
        Ok(record)
    }
}

impl From<nom::Err<nom::error::Error<&str>>> for HepMCError {
    fn from(source: nom::Err<nom::error::Error<&str>>) -> Self {
        Self::ParseError(source.to_string())
    }
}

fn double_entry(line: &str) -> IResult<&str, f64> {
    preceded(space1, double)(line)
}

fn any_entry(line: &str) -> IResult<&str, &str> {
    preceded(space1, non_space)(line)
}

fn u32_entry(line: &str) -> IResult<&str, u32> {
    preceded(space1, u32)(line)
}

fn i32_entry(line: &str) -> IResult<&str, i32> {
    preceded(space1, i32)(line)
}

fn string_entry(line: &str) -> IResult<&str, &str> {
    preceded(space1, string)(line)
}

fn string(line: &str) -> IResult<&str, &str> {
    delimited(char('"'), take_until("\""), char('"'))(line)
}

fn non_space(line: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_ascii_whitespace())(line)
}
