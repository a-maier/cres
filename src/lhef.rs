use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use audec::auto_decompress;
use log::trace;
use noisy_float::prelude::*;
use nom::{
    character::complete::{i32, space0, u32},
    multi::count,
    sequence::preceded,
    IResult,
};
use particle_id::ParticleID;

use crate::{
    compression::{compress_writer, Compression},
    event::{Event, EventBuilder, Weights},
    hepmc2::update_central_weight,
    io::{
        Converter, CreateError, ErrorKind, EventFileReader, EventRecord,
        FileIOError, ReadError, WriteError,
    },
    parsing::{any_entry, double_entry, i32_entry, non_space},
    traits::{Rewind, UpdateWeights},
    util::take_chars,
};

/// Reader from a (potentially compressed) Les Houches Event File
pub struct FileReader {
    source_path: PathBuf,
    source: Box<dyn BufRead>,
    header: Vec<u8>,
}

impl FileReader {
    /// Construct a reader from the given (potentially compressed) Les Houches Event File
    pub fn try_new(source_path: PathBuf) -> Result<Self, CreateError> {
        let (header, source) = init_source(&source_path)?;
        Ok(Self {
            source_path,
            source,
            header,
        })
    }

    fn read_raw(&mut self) -> Option<Result<String, ReadError>> {
        let mut record = Vec::new();
        while !record.ends_with(b"</event>") {
            match self.source.read_until(b'>', &mut record) {
                Ok(0) => return None, // TODO: check incomplete record
                Ok(_) => {}
                Err(err) => return Some(Err(err.into())),
            }
        }
        let record = String::from_utf8(record).unwrap();
        trace!("Read Les Houches Event record:\n{record}");
        Some(Ok(record))
    }
}

impl EventFileReader for FileReader {
    fn path(&self) -> &Path {
        self.source_path.as_path()
    }

    fn header(&self) -> &[u8] {
        &self.header
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
        self.read_raw().map(|r| r.map(EventRecord::LHEF))
    }
}

/// I/O from and to (potentially compressed) Les Houches Event Files
pub struct FileIO {
    reader: FileReader,
    sink_path: PathBuf,
    sink: Box<dyn Write>,
    _weight_names: Vec<String>,
}

impl FileIO {
    /// Construct an I/O object from the given (potentially compressed) Les Houches Event Files
    pub fn try_new(
        // TODO: use builder pattern instead?
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
            let (rest, _non_weight) = non_weight_entries(&record)
                .map_err(|_| parse_err("entries before weight", &record))?;

            let weights_start = record.len() - rest.len();
            update_central_weight(
                &mut record,
                weights_start,
                weights.central(),
            )?;

            #[cfg(feature = "multiweight")]
            if weights.len() > 1 {
                unimplemented!("Multiple weights in LHEF")
            }
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

fn init_source(
    source: impl AsRef<Path>,
) -> Result<(Vec<u8>, Box<dyn BufRead>), CreateError> {
    use CreateError::*;

    let source = File::open(source).map_err(OpenInput)?;
    let mut buf = auto_decompress(BufReader::new(source));

    // read until start of first event
    let mut header = Vec::new();
    while !header.ends_with(b"\n</init>") {
        if buf.read_until(b'>', &mut header).map_err(Read)? == 0 {
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
    type Error = ReadError;

    fn parse_lhef(&self, mut record: &str) -> Result<Event, Self::Error> {
        use ReadError::*;
        const STATUS_OUTGOING: i32 = 1;

        let parse_err =
            |entry, record: &str| ParseEntry(entry, take_chars(record, 100));

        // TODO: multiple weights
        record = record.trim_start();
        let Some(line_end) = record.find('\n') else {
            return Err(FindEntry("line break", record.to_string()));
        };
        record = &record[(1 + line_end)..];
        let (rest, nparticles): (_, u32) =
            u32_entry0(record).map_err(|_| parse_err("NUP entry", record))?;
        let nparticles = nparticles as usize;
        let mut event = EventBuilder::with_capacity(nparticles - 2);
        let (rest, _idrup) =
            any_entry(rest).map_err(|_| parse_err("IDRUP entry", rest))?;
        let (rest, wt) =
            double_entry(rest).map_err(|_| parse_err("XWGTUP entry", rest))?;
        event.add_weight(n64(wt));
        for line in rest.lines().skip(1).take(nparticles) {
            let (rest, id) =
                i32_entry0(line).map_err(|_| parse_err("IDUP entry", line))?;
            let id = ParticleID::new(id);
            let (rest, status) =
                i32_entry(rest).map_err(|_| parse_err("ISTUP entry", rest))?;
            if status != STATUS_OUTGOING {
                continue;
            }
            // ignore decay parents & colour
            let (rest, _) = count(any_entry, 4)(rest).map_err(|_| {
                parse_err("entries before particle momentum", rest)
            })?;
            let (rest, px) =
                double_entry(rest).map_err(|_| parse_err("px entry", rest))?;
            let (rest, py) =
                double_entry(rest).map_err(|_| parse_err("py entry", rest))?;
            let (rest, pz) =
                double_entry(rest).map_err(|_| parse_err("py entry", rest))?;
            let (_, e) = double_entry(rest)
                .map_err(|_| parse_err("energy entry", rest))?;
            event.add_outgoing(id, [n64(e), n64(px), n64(py), n64(pz)].into());
        }

        Ok(event.build())
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
        self.finish_weight_update()?;
        Ok(weights.len())
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights,
    ) -> Result<bool, Self::Error> {
        let res = self.update_next_weights_helper(weights);
        self.into_io_error(res)
    }

    fn finish_weight_update(&mut self) -> Result<(), Self::Error> {
        let res = self.sink.write_all(b"\n</LesHouchesEvents>\n");
        self.into_io_error(res.map_err(WriteError::IO))?;
        Ok(())
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

pub(crate) fn u32_entry0(line: &str) -> IResult<&str, u32> {
    preceded(space0, u32)(line)
}

pub(crate) fn i32_entry0(line: &str) -> IResult<&str, i32> {
    preceded(space0, i32)(line)
}
