use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use audec::auto_decompress;
use log::trace;
use noisy_float::prelude::*;
use nom::{multi::count, IResult, Parser};
use particle_id::ParticleID;

use crate::{
    compression::{compress_writer, Compression},
    event::{Event, EventBuilder, Weights},
    io::{
        Converter, CreateError, ErrorKind, EventFileReader, EventRecord,
        FileIOError, ReadError, WriteError,
    },
    parsing::{any_entry, double_entry, i32_entry, u32_entry},
    traits::{Rewind, UpdateWeights},
    util::take_chars,
};

/// Reader from a (potentially compressed) HepMC2 event file
pub struct FileReader {
    source_path: PathBuf,
    source: Box<dyn BufRead>,
    header: Vec<u8>,
}

impl FileReader {
    /// Construct a reader from the given (potentially compressed) HepMC2 event file
    pub fn try_new(source_path: PathBuf) -> Result<Self, CreateError> {
        let (header, source) = init_source(&source_path)?;
        Ok(Self {
            source_path,
            source,
            header,
        })
    }

    fn read_raw(&mut self) -> Option<Result<String, ReadError>> {
        let mut record = vec![b'E'];
        while !record.ends_with(b"\nE") {
            match self.source.read_until(b'E', &mut record) {
                Ok(0) => {
                    if record.len() > 1 {
                        let record = String::from_utf8(record).unwrap();
                        assert!(record.starts_with('E'));
                        trace!("Read HepMC record:\n{record}");
                        return Some(Ok(record));
                    } else {
                        return None;
                    }
                }
                Ok(_) => {}
                Err(err) => return Some(Err(err.into())),
            }
        }
        record.pop();
        let record = String::from_utf8(record).unwrap();
        assert!(record.starts_with('E'));
        trace!("Read HepMC record:\n{record}");
        Some(Ok(record))
    }
}

impl EventFileReader for FileReader {
    fn path(&self) -> &Path {
        self.source_path.as_path()
    }

    fn header(&self) -> &[u8] {
        self.header.as_slice()
    }
}

impl Rewind for FileReader {
    type Error = CreateError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        (_, self.source) = init_source(&self.source_path)?;

        Ok(())
    }
}

impl Iterator for FileReader {
    type Item = Result<EventRecord, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.read_raw().map(|r| r.map(EventRecord::HepMC))
    }
}

fn init_source(
    source: impl AsRef<Path>,
) -> Result<(Vec<u8>, Box<dyn BufRead>), CreateError> {
    use CreateError::*;

    let source = File::open(source).map_err(OpenInput)?;
    let mut buf = auto_decompress(BufReader::new(source));

    // read until start of first event
    let mut header = Vec::new();
    while !header.ends_with(b"\nE") {
        if buf.read_until(b'E', &mut header).map_err(Read)? == 0 {
            break;
        }
    }
    header.pop();
    Ok((header, buf))
}

/// I/O from and to (potentially compressed) HepMC2 event files
pub struct FileIO {
    reader: FileReader,
    sink_path: PathBuf,
    sink: Box<dyn Write>,
    _weight_names: Vec<String>,
}

impl FileIO {
    /// Construct a I/O object from the given (potentially compressed) HepMC2 event files
    pub fn try_new(
        source_path: PathBuf,
        sink_path: PathBuf,
        compression: Option<Compression>,
        _weight_names: Vec<String>,
    ) -> Result<Self, CreateError> {
        use CreateError::*;
        let reader = FileReader::try_new(source_path)?;
        let outfile = File::create(&sink_path).map_err(CreateTarget)?;
        let sink = BufWriter::new(outfile);
        let mut sink =
            compress_writer(sink, compression).map_err(CompressTarget)?;
        sink.write_all(reader.header()).map_err(Write)?;

        Ok(FileIO {
            reader,
            sink_path,
            sink,
            _weight_names,
        })
    }

    #[allow(clippy::wrong_self_convention)]
    fn into_io_error<T, E: Into<ErrorKind>>(
        &self,
        res: Result<T, E>,
    ) -> Result<T, FileIOError> {
        res.map_err(|err| {
            FileIOError::new(
                self.reader.path().to_path_buf(),
                self.sink_path.clone(),
                err.into(),
            )
        })
    }

    fn update_next_weights_helper(
        &mut self,
        weights: &Weights,
    ) -> Result<bool, ErrorKind> {
        use ErrorKind::*;
        use ReadError::ParseEntry;
        use WriteError::IO;

        let parse_err = |what, record: &str| {
            Read(ParseEntry(what, take_chars(record, 100)))
        };

        let Some(record) = self.reader.read_raw() else {
            return Ok(false);
        };
        let mut record = record?;

        if !weights.is_empty() {
            debug_assert!(record.starts_with('E'));
            let (weight_entries, _non_weight) = non_weight_entries(&record)
                .map_err(|_| parse_err("entries before weight", &record))?;
            let (rest, _nweights) =
                u32_entry(weight_entries).map_err(|_| {
                    parse_err("number of weights entry", weight_entries)
                })?;

            let weights_start = record.len() - rest.len();
            update_central_weight(
                &mut record,
                weights_start,
                weights.central(),
            )?;

            #[cfg(feature = "multiweight")]
            update_named_weights(
                &mut record,
                weights_start,
                _nweights as usize,
                &self._weight_names,
                weights,
            )?;
        }
        self.sink.write_all(record.as_bytes()).map_err(IO)?;
        Ok(true)
    }
}

impl Rewind for FileIO {
    type Error = FileIOError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        let res = self.reader.rewind();
        self.into_io_error(res)
    }
}

impl Iterator for FileIO {
    type Item = Result<EventRecord, FileIOError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.next().map(|r| self.into_io_error(r))
    }
}

impl UpdateWeights for FileIO {
    type Error = FileIOError;

    fn update_all_weights(
        &mut self,
        weights: &[Weights],
    ) -> Result<usize, Self::Error> {
        self.rewind()?;
        for (n, weight) in weights.iter().enumerate() {
            if !self.update_next_weights(weight)? {
                return Ok(n);
            }
        }
        Ok(weights.len())
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights,
    ) -> Result<bool, Self::Error> {
        let res = self.update_next_weights_helper(weights);
        self.into_io_error(res)
    }
}

/// Parser for HepMC event records
pub trait HepMCParser {
    /// Error parsing HepMC event record
    type Error;

    /// Parse HepMC event record
    fn parse_hepmc(&self, record: &str) -> Result<Event, Self::Error>;
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum EnergyUnit {
    MeV,
    #[default]
    GeV,
}

impl HepMCParser for Converter {
    type Error = ReadError;

    fn parse_hepmc(&self, mut record: &str) -> Result<Event, Self::Error> {
        use ReadError::*;

        let mut event = EventBuilder::new();
        let (weight, _weights, rest) = extract_weights(record)?;
        event.add_weight(n64(weight));
        record = rest;

        #[allow(non_snake_case)]
        let mut energy_unit = EnergyUnit::GeV;
        while let Some(pos) = record.find('\n') {
            record = &record[(pos + 1)..];
            match record.as_bytes().first() {
                Some(b'V') | Some(b'F') | Some(b'H') | Some(b'C') => {}
                #[cfg(not(feature = "multiweight"))]
                Some(b'N') => {}
                #[cfg(feature = "multiweight")]
                Some(b'N') => {
                    record = parse_weight_names_line(
                        self, record, &_weights, &mut event,
                    )?
                }
                Some(b'P') => record = parse_particle_line(record, &mut event)?,
                Some(b'U') => (energy_unit, record) = parse_units_line(record)?,
                _ => {
                    if !record.trim().is_empty() {
                        return Err(UnrecognisedEntry(
                            "start",
                            take_chars(record, 100),
                        ));
                    }
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

#[cfg(feature = "multiweight")]
fn update_named_weights(
    record: &mut String,
    weights_start: usize,
    nweights: usize,
    weight_names: &[String], // TODO: maybe a set is better
    weights: &Weights,
) -> Result<(), ReadError> {
    use ReadError::*;
    assert_eq!(weights.len(), weight_names.len() + 1);

    if weight_names.is_empty() {
        return Ok(());
    }
    let start = record.find("\nN").ok_or_else(|| {
        FindEntry(
            "weight name entry (line starting with 'N')",
            record.to_string(),
        )
    })?;

    let names = &record[(start + 2)..];
    let (names, nnames) = u32_entry(names).map_err(|_| {
        ParseEntry("number of weight names entry", take_chars(names, 100))
    })?;
    let mut weight_pos = Vec::with_capacity(weight_names.len());
    let mut rest = names;
    for nentry in 0..(nnames as usize) {
        let name;
        (rest, name) = string_entry(rest).map_err(|_| {
            ParseEntry("weight name entry", take_chars(rest, 100))
        })?;
        if weight_names.iter().any(|n| n == name) {
            weight_pos.push(nentry);
        }
    }

    let mut weight_entries = Vec::from_iter(
        record[weights_start..]
            .split_ascii_whitespace()
            .take(nweights)
            .map(|w| w.to_string()),
    );
    for (idx, weight) in weight_pos.iter().zip(weights.iter().skip(1)) {
        weight_entries[*idx] = weight.to_string();
    }

    let start = weights_start + 1;
    let line_end = record[start..].find('\n').unwrap();
    let end = start + line_end;
    // there are no entries in the E line after the weights, so this is safe
    record.replace_range(start..end, &weight_entries.join(" "));
    Ok(())
}

pub(crate) fn update_central_weight(
    record: &mut String,
    entry_pos: usize,
    weight: N64,
) -> Result<(), ReadError> {
    use ReadError::*;

    let (after_entry, _) = any_entry(&record[entry_pos..]).map_err(|_| {
        ParseEntry("central weight entry", take_chars(record, 100))
    })?;
    // +1 to ensure we skip one space
    let start = entry_pos + 1;
    let end = record.len() - after_entry.len();
    record.replace_range(start..end, &weight.to_string());
    Ok(())
}

fn parse_units_line(record: &str) -> Result<(EnergyUnit, &str), ReadError> {
    use ReadError::*;

    debug_assert!(record.starts_with('U'));
    let (rest, energy) = any_entry(&record[1..]).map_err(|_| {
        ParseEntry("energy unit entry", take_chars(record, 100))
    })?;
    let energy = match energy {
        "GEV" => EnergyUnit::GeV,
        "MEV" => EnergyUnit::MeV,
        _ => {
            return Err(InvalidEntry {
                value: energy.to_string(),
                entry: "energy unit entry",
                record: take_chars(record, 100),
            })
        }
    };
    Ok((energy, rest))
}

fn parse_particle_line<'a>(
    record: &'a str,
    event: &mut EventBuilder,
) -> Result<&'a str, ReadError> {
    const HEPMC_OUTGOING: i32 = 1;

    let parse_err =
        |entry| ReadError::ParseEntry(entry, take_chars(record, 100));

    debug_assert!(record.starts_with('P'));
    let (rest, _barcode) =
        any_entry(&record[1..]).map_err(|_| parse_err("barcode entry"))?;
    let (rest, id) =
        i32_entry(rest).map_err(|_| parse_err("particle id entry"))?;
    let (rest, px) = double_entry(rest).map_err(|_| parse_err("px entry"))?;
    let (rest, py) = double_entry(rest).map_err(|_| parse_err("py entry"))?;
    let (rest, pz) = double_entry(rest).map_err(|_| parse_err("pz entry"))?;
    let (rest, e) =
        double_entry(rest).map_err(|_| parse_err("energy entry"))?;
    let (rest, _m) = any_entry(rest).map_err(|_| parse_err("mass entry"))?;
    let (rest, status) =
        i32_entry(rest).map_err(|_| parse_err("particle status entry"))?;
    if status != HEPMC_OUTGOING {
        return Ok(rest);
    }
    event.add_outgoing(
        ParticleID::new(id),
        [n64(e), n64(px), n64(py), n64(pz)].into(),
    );
    Ok(rest)
}

fn extract_weights(record: &str) -> Result<(f64, Vec<f64>, &str), ReadError> {
    use ReadError::*;

    let parse_err =
        |what, record: &str| ParseEntry(what, take_chars(record, 100));

    if !record.starts_with('E') {
        return Err(UnrecognisedEntry(
            "start of record",
            take_chars(record, 100),
        ));
    }
    let (rest, _) = non_weight_entries(record)
        .map_err(|_| parse_err("entries before weight", record))?;

    let (rest, nweights) = u32_entry(rest)
        .map_err(|_| parse_err("number of weights entry", rest))?;

    let res = if cfg!(feature = "multiweight") {
        let (rest, weights) = count(double_entry, nweights as usize)
            .parse(rest)
            .map_err(|_| parse_err("event weight entries", rest))?;
        (weights[0], weights, rest)
    } else {
        let (rest, weight) = double_entry(rest)
            .map_err(|_| parse_err("event weight entry", rest))?;
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
) -> Result<&'a str, ReadError> {
    use std::collections::HashMap;
    use ReadError::*;

    let parse_err =
        |what, record: &str| ParseEntry(what, take_chars(record, 100));

    let weight_names = converter.weight_names();
    if weight_names.is_empty() {
        return Ok(record);
    }
    let mut weight_seen: HashMap<_, _> =
        weight_names.iter().map(|n| (n.as_str(), false)).collect();
    let (names, nnames) = u32_entry(&record[1..])
        .map_err(|_| parse_err("number of weight names entry", record))?;
    record = names;
    for weight in all_weights.iter().take(nnames as usize) {
        let name;
        (record, name) = string_entry(record)
            .map_err(|_| parse_err("weight name entry", record))?;
        if let Some(seen) = weight_seen.get_mut(name) {
            *seen = true;
            event.add_weight(n64(*weight));
        }
    }
    let missing =
        weight_seen
            .into_iter()
            .find_map(|(name, seen)| if seen { None } else { Some(name) });
    if let Some(missing) = missing {
        Err(FindWeight(missing.to_owned(), names.to_owned()))
    } else {
        Ok(record)
    }
}

fn non_weight_entries(line: &str) -> IResult<&str, &str> {
    debug_assert!(line.starts_with('E'));
    // ignore first 10 entries
    let (rest, _) = count(any_entry, 10).parse(&line[1..])?;
    let (rest, nrandom_states) = u32_entry(rest)?;
    // ignore random states
    let (rest, _) = count(any_entry, nrandom_states as usize).parse(rest)?;
    let (parsed, rest) = line.split_at(line.len() - rest.len());
    Ok((rest, parsed))
}

#[cfg(feature = "multiweight")]
fn string_entry(line: &str) -> IResult<&str, &str> {
    use nom::{character::complete::space1, sequence::preceded, Parser};
    preceded(space1, string).parse(line)
}

#[cfg(feature = "multiweight")]
fn string(line: &str) -> IResult<&str, &str> {
    use nom::{
        bytes::complete::take_until, character::complete::char,
        sequence::delimited,
    };
    delimited(char('"'), take_until("\""), char('"')).parse(line)
}
