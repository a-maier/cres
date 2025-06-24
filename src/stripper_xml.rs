use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Display},
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use audec::auto_decompress;
use log::{debug, trace};
use noisy_float::prelude::*;
use nom::{
    bytes::complete::tag,
    character::complete::{
        char, i32, multispace0, multispace1, space0, space1, u64,
    },
    combinator::{all_consuming, opt, recognize},
    sequence::preceded,
    IResult, Parser,
};
use particle_id::ParticleID;
use quick_xml::events::attributes::Attribute;
use stripper_xml::normalization::Normalization;
use thiserror::Error;

use crate::{
    compression::{compress_writer, Compression},
    event::{Event, EventBuilder, Weights},
    four_vector::FourVector,
    io::{
        Converter, CreateError, ErrorKind, EventFileReader, EventRecord,
        EventTypeInfo, FileIOError, ReadError, Utf8Error, WriteError,
    },
    traits::{Rewind, UpdateWeights},
    util::{take_chars, trim_ascii_start},
};

/// Reader from a (potentially compressed) Les Houches Event File
pub struct FileReader {
    source_path: PathBuf,
    source: Box<dyn BufRead>,
    rem_subevents: usize,
    header: Vec<u8>,
}

impl FileReader {
    /// Construct a reader from the given (potentially compressed) HepMC2 event file
    pub fn try_new(source_path: PathBuf) -> Result<Self, CreateError> {
        use crate::stripper_xml::CreateError::XMLError;

        let (header, source) = init_source(&source_path)?;
        let header_info = extract_xml_info(header.as_slice())?;
        let XMLTag::Eventrecord { nsubevents, .. } = header_info else {
            return Err(XMLError(Error::BadTag(header_info.to_string())));
        };

        let rem_subevents = nsubevents as usize;
        Ok(Self {
            source_path,
            source,
            rem_subevents,
            header,
        })
    }

    fn read_raw(&mut self) -> Option<Result<String, ReadError>> {
        let mut record = b"<se".to_vec();
        loop {
            match self.source.read_until(b'e', &mut record) {
                Ok(0) => {
                    if record.len() > 3 {
                        break;
                    } else {
                        return None;
                    }
                }
                Ok(_) => {
                    if record.ends_with(b"<se") {
                        record.truncate(record.len() - 3);
                        break;
                    }
                }
                Err(err) => return Some(Err(err.into())),
            }
        }

        let record = match String::from_utf8(record) {
            Ok(record) => record,
            Err(err) => return Some(Err(Utf8Error::from(err).into())),
        };

        assert!(record.starts_with("<se"));

        trace!("Read STRIPPER XML record:\n{record}");

        self.rem_subevents = self.rem_subevents.saturating_sub(1);
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
        self.read_raw().map(|r| {
            r.map(|record| EventRecord::StripperXml {
                record,
                weight_names: Vec::new(),
                weight_scale: 1.0,
            })
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.rem_subevents, Some(self.rem_subevents))
    }
}

/// I/O using (potentially compressed) STRIPPER XML files
pub struct FileIO {
    reader: FileReader,
    sink_path: PathBuf,
    sink: Box<dyn Write>,
    weights_to_resample: Vec<String>,
    weight_scale: f64,
    discard_weightless: bool,
    weight_entries: Vec<String>,
}

impl FileIO {
    /// Construct a STRIPPER XML event I/O object
    ///
    /// Construct an I/O object using the given (potentially compressed)
    /// STRIPPER XMLs file with the given information for
    /// channel-specific scale factors
    pub fn try_new(
        // TODO: use builder pattern instead?
        source_path: PathBuf,
        sink_path: PathBuf,
        compression: Option<Compression>,
        weights_to_resample: Vec<String>,
        discard_weightless: bool,
        scaling: &HashMap<String, EventTypeInfo>,
    ) -> Result<Self, CreateError> {
        use CreateError::*;

        let reader = FileReader::try_new(source_path)?;
        let outfile = File::create(&sink_path).map_err(CreateTarget)?;
        let sink = BufWriter::new(outfile);
        let mut sink =
            compress_writer(sink, compression).map_err(CompressTarget)?;
        sink.write_all(reader.header()).map_err(Write)?;
        let header_info = extract_xml_info(reader.header())?;
        let XMLTag::Eventrecord { name, .. } = header_info else {
            return Err(XMLError(Error::BadTag(header_info.to_string())));
        };

        let Some(info) = scaling.get(&name) else {
            return Err(XMLError(Error::MissingScaling(name)));
        };

        let weight_scale = info.scale;
        let weight_entries = info.weight_names.clone();

        Ok(FileIO {
            reader,
            sink_path,
            sink,
            discard_weightless,
            weights_to_resample,
            weight_scale,
            weight_entries,
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
        use ReadError::{FindEntry, ParseEntry};
        use WriteError::IO;

        let parse_err = |what, record: &str| {
            Read(ParseEntry(what, take_chars(record, 100)))
        };

        let Some(record) = self.reader.read_raw() else {
            return Ok(false);
        };
        let mut record = record?;

        if !weights.is_empty() {
            if self.discard_weightless && weights.central() == 0. {
                return Ok(true);
            }

            let weight = weights.central() / self.weight_scale;

            // TODO: code duplication with `rescale_weight`
            let (rest, start) = weight_start(record.as_str())
                .map_err(|_| parse_err("start of event record", &record))?;
            let (rest, old_weight) =
                double(rest).map_err(|_| parse_err("weight entry", rest))?;

            let start = start.len();
            let end = record.len() - rest.len();
            let wt_str = weight.to_string();
            record.replace_range(start..end, &wt_str);
            trace!("replaced weight: {old_weight} -> {weight}");

            #[cfg(feature = "multiweight")]
            if weights.len() > 1 {
                let start = start + wt_str.len();
                let rest = &record[start..];
                let mut weights =
                    weights.iter().skip(1).map(|&wt| wt / self.weight_scale);
                let Some(s) = rest.find("<rw ") else {
                    return Err(Read(FindEntry("reweight entry", record)));
                };
                let start = start + s;
                let rest = &record[start..];
                let (_rest, rwtag) = reweight_start(rest)
                    .map_err(|_| parse_err("reweight entry", rest))?;
                let mut start = start + rwtag.len();
                for entry in &self.weight_entries {
                    let rest = &record[start..];
                    let (_rest, old) = recognize(double).parse(rest)
                        .map_err(|_| parse_err("reweight entry", rest))?;
                    start += if self.weights_to_resample.contains(entry) {
                        let wt_str = weights.next().unwrap().to_string();
                        record.replace_range(start..(start + old.len()), &wt_str);
                        wt_str.len()
                    } else {
                        old.len()
                    };
                    let rest = &record[start..];
                    let (_rest, sep) = ws_comma(rest).unwrap();
                    start += sep.len();
                }
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
        let res = self.reader.read_raw().map(|r| {
            r.map(|record| EventRecord::StripperXml {
                record,
                weight_names: self.weight_entries.clone(),
                weight_scale: self.weight_scale,
            })
        });
        res.map(|n| self.into_io_error(n))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.reader.size_hint()
    }
}

pub(crate) fn extract_info<I, P, Q>(
    paths: I,
) -> Result<(Vec<(PathBuf, Q)>, HashMap<String, EventTypeInfo>), CreateError>
where
    I: IntoIterator<Item = (P, Q)>,
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    use CreateError::*;

    #[derive(Debug, Default)]
    struct RawInfo {
        raw_scale: (f64, u64),
        weight_names: Vec<String>,
    }

    impl From<RawInfo> for EventTypeInfo {
        fn from(source: RawInfo) -> Self {
            let RawInfo {
                raw_scale,
                weight_names,
            } = source;
            let (raw_scale, nevents) = raw_scale;
            let scale = raw_scale / (nevents as f64);
            Self {
                scale,
                weight_names,
            }
        }
    }

    let mut event_files = Vec::new();
    let mut rescale: HashMap<_, RawInfo> = HashMap::new();
    for (path, out) in paths {
        let path = path.as_ref();
        let file = File::open(path).map_err(OpenInput)?;
        let mut r = auto_decompress(BufReader::new(file));
        if let Ok(buf) = r.fill_buf() {
            let buf = trim_ascii_start(buf);
            if buf.starts_with(b"<?xml") {
                debug!("extracting scaling information from {path:?}");
                let tag = extract_xml_info(r)?;
                match tag {
                    XMLTag::Normalization {
                        name,
                        scale,
                        weight_names,
                    } => {
                        let entry = rescale.entry(name).or_default();
                        entry.raw_scale.0 = scale;
                        entry.weight_names = weight_names;
                        // don't add to vec of event files
                    }
                    XMLTag::Eventrecord { name, nevents, .. } => {
                        let entry = rescale.entry(name).or_default();
                        entry.raw_scale.1 += nevents;
                        event_files.push((path.to_owned(), out))
                    }
                }
            } else {
                // not a STRIPPER XML file
                event_files.push((path.to_owned(), out));
            }
        } else {
            event_files.push((path.to_owned(), out))
        }
    }
    let rescale = rescale
        .into_iter()
        .map(|(name, raw_info)| (name, raw_info.into()))
        .collect();
    Ok((event_files, rescale))
}

pub(crate) fn extract_xml_info(r: impl BufRead) -> Result<XMLTag, CreateError> {
    use quick_xml::events::Event;
    use Error::*;

    let mut buf = Vec::new();
    let mut reader = quick_xml::Reader::from_reader(r);
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"Normalization" => {
                        // restore tag delimiters
                        buf.insert(0, b'<');
                        buf.push(b'>');
                        let rest = reader.into_inner();
                        let all = buf.chain(rest);
                        let norm: Normalization =
                            quick_xml::de::from_reader(all)
                                .map_err(NormalizationDeser)?;
                        return Ok(XMLTag::Normalization {
                            name: norm.contribution.name,
                            scale: norm.contribution.xsection.0[0],
                            weight_names: norm.contribution.rw.rwentry,
                        });
                    }
                    b"Eventrecord" => {
                        let mut name = None;
                        let mut nevents = None;
                        let mut nsubevents = None;
                        let mut alpha_s_power = None;
                        let attributes =
                            e.attributes().filter_map(|a| match a {
                                Ok(a) => Some(a),
                                Err(_) => None,
                            });
                        for attr in attributes {
                            let attr_err = |attr, val: &Attribute<'_>| {
                                let val: &[u8] = val.value.as_ref();
                                Error::AttrType {
                                    attr,
                                    val: String::from_utf8_lossy(val)
                                        .to_string(),
                                    wanted: "64bit floating-point number",
                                }
                            };
                            match attr.key.0 {
                                b"nevents" => {
                                    let (_, val) =
                                        parse_u64(&attr).map_err(|_| {
                                            attr_err("nevents", &attr)
                                        })?;
                                    nevents = Some(val);
                                }
                                b"nsubevents" => {
                                    let (_, val) =
                                        parse_u64(&attr).map_err(|_| {
                                            attr_err("nsubevents", &attr)
                                        })?;
                                    nsubevents = Some(val);
                                }
                                b"name" => name = Some(to_string(attr.value)?),
                                b"as" => {
                                    let (_, val) = parse_u64(&attr)
                                        .map_err(|_| attr_err("as", &attr))?;
                                    alpha_s_power = Some(val);
                                }
                                _ => {}
                            }
                        }
                        let Some(name) = name else {
                            return Err(NoEventrecordAttr("name").into());
                        };
                        let Some(nsubevents) = nsubevents else {
                            return Err(NoEventrecordAttr("nsubevents").into());
                        };
                        let Some(nevents) = nevents else {
                            return Err(NoEventrecordAttr("nevents").into());
                        };
                        let Some(alpha_s_power) = alpha_s_power else {
                            return Err(NoEventrecordAttr("as").into());
                        };
                        return Ok(XMLTag::Eventrecord {
                            alpha_s_power,
                            name,
                            nevents,
                            nsubevents,
                        });
                    }
                    name => {
                        let name = std::str::from_utf8(name)
                            .map_err(Utf8Error::Utf8)?;
                        return Err(BadTag(name.to_owned()).into());
                    }
                }
            }
            Ok(Event::Decl(_) | Event::Text(_)) => {} // ignore,
            _ => return Err(NoTag.into()),
        }
    }
}

fn parse_u64<'a, 'b: 'a>(attr: &'a Attribute<'b>) -> IResult<&'a [u8], u64> {
    all_consuming(u64).parse(attr.value.as_ref())
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub(crate) enum XMLTag {
    Normalization {
        name: String,
        scale: f64,
        weight_names: Vec<String>,
    },
    Eventrecord {
        alpha_s_power: u64,
        name: String,
        nevents: u64,
        nsubevents: u64,
    },
}

impl Display for XMLTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            XMLTag::Normalization {
                name,
                scale,
                weight_names: _,
            } => write!(f, r#"<Normalization name="{name}" scale="{scale}">"#),
            XMLTag::Eventrecord {
                alpha_s_power,
                name,
                nevents,
                nsubevents,
            } => write!(
                f,
                r#"<Eventrecord nevents="{nevents}" nsubevents="{nsubevents}" as="{alpha_s_power}" name="{name}"">"#
            ),
        }
    }
}

fn to_string(value: Cow<[u8]>) -> Result<String, Utf8Error> {
    match value {
        Cow::Borrowed(s) => Ok(std::str::from_utf8(s)?.to_owned()),
        Cow::Owned(s) => Ok(String::from_utf8(s).map_err(|e| e.utf8_error())?),
    }
}

/// STRIPPER XML Error
#[derive(Debug, Error)]
pub enum Error {
    /// Missing XML tag
    #[error("File does not start with an XML tag")]
    NoTag,
    /// Missing scaling
    #[error("No scaling information found for {0}")]
    MissingScaling(String),
    /// Unsupported XML tag
    #[error("File starts with an unsupported XML tag `{0}`")]
    BadTag(String),
    /// Missing attribute in event record
    #[error("XML tag `Eventrecord` does not have a `{0}` attribute")]
    NoEventrecordAttr(&'static str),
    /// Wrong attribute type
    #[error("Value {val} of attribute `{attr}` is not a `{wanted}`")]
    AttrType {
        /// Attribute name
        attr: &'static str,
        /// Attribute value
        val: String,
        /// Wanted type
        wanted: &'static str,
    },
    /// Deserialisation error for [Normalization]
    #[error("Failed to deserialise `Normalization`")]
    NormalizationDeser(#[from] quick_xml::DeError),
    /// Unclosed tag
    #[error("Tag {0} is not closed in {1}")]
    UnclosedTag(String, String),
    /// Missing end tag
    #[error("Failed to find end of tag {0} in {1}")]
    IncompleteTag(&'static str, String),
}

fn init_source(
    source: impl AsRef<Path>,
) -> Result<(Vec<u8>, Box<dyn BufRead>), CreateError> {
    use CreateError::*;

    let source = File::open(source).map_err(OpenInput)?;
    let mut buf = auto_decompress(BufReader::new(source));

    // read until start of first event
    let mut header = Vec::new();
    while !header.ends_with(b"<se") {
        if buf.read_until(b'e', &mut header).map_err(Read)? == 0 {
            break;
        }
    }
    header.truncate(header.len() - b"<se".len());
    Ok((header, buf))
}

/// Parser for STRIPPER XML event records
pub trait StripperXmlParser {
    /// Error parsing STRIPPER XML event record
    type Error;

    /// Parse STRIPPER XML event record
    fn parse_stripper_xml(
        &self,
        record: &str,
        weight_scale: f64,
        weight_names: &[String],
    ) -> Result<Event, Self::Error>;
}

pub(crate) const PID_X1: ParticleID = ParticleID::new(99);
pub(crate) const PID_X2: ParticleID = ParticleID::new(100);

impl StripperXmlParser for Converter {
    type Error = ReadError;

    fn parse_stripper_xml(
        &self,
        record: &str,
        weight_scale: f64,
        weight_names: &[String],
    ) -> Result<Event, Self::Error> {
        use ReadError::*;

        let parse_err =
            |what, record: &str| ParseEntry(what, take_chars(record, 100));

        const STATUS_OUTGOING: i32 = 0;

        let mut event = EventBuilder::new();

        let (rest, _start) = weight_start(record)
            .map_err(|_| parse_err("start of event record", record))?;

        let (rest, weight) =
            double(rest).map_err(|_| parse_err("weight entry", rest))?;

        event.add_weight(n64(weight * weight_scale));

        let Some(tag_end) = rest.find('>') else {
            return Err(
                Error::IncompleteTag("<se>", take_chars(record, 100)).into()
            );
        };
        let mut rest = &rest[(tag_end + 1)..];

        while let Ok((r, _)) = particle_start(rest) {
            let (r, status) = particle_status(r)
                .map_err(|_| parse_err("particle status entry", r))?;

            if status != STATUS_OUTGOING {
                let Some(particle_end) = r.find("</p>") else {
                    return Err(Error::UnclosedTag(
                        "<p".to_string(),
                        take_chars(rest, 100),
                    )
                    .into());
                };
                rest = &r[(particle_end + "</p>".len())..];
                continue;
            }

            type NomErr<'a> = nom::Err<nom::error::Error<&'a str>>;
            let (r, pid) = particle_id(r)
                .map_err(|_| parse_err("particle id entry", r))?;
            let (r, _) = tag("\">").parse(r).map_err(|_err: NomErr<'_>| {
                Error::IncompleteTag("<p>", take_chars(r, 100))
            })?;
            let (r, p) = particle_momentum(&r[1..])
                .map_err(|_| parse_err("particle momentum entry", r))?;
            event.add_outgoing(pid, p);
            (rest, _) = tag("</p>").parse(r).map_err(|_err: NomErr<'_>| {
                Error::UnclosedTag("<p".to_string(), take_chars(r, 100))
            })?;
        }

        let (mut rest, _) = reweight_start(rest)
            .map_err(|_| parse_err("reweight entry", rest))?;

        // TODO: command line option for including x1, x2?
        let mut x1 = None;
        let mut x2 = None;
        for name in weight_names {
            let (r, wt) = preceded(opt(char(',')), double).parse(rest)
                .map_err(|_| parse_err("reweight entry", rest))?;
            rest = r;
            if self.weight_names().contains(name) {
                event.add_weight(n64(wt * weight_scale));
            }
            match name.as_str() {
                "x1" => x1 = Some(wt),
                "x2" => x2 = Some(wt),
                _ => {}
            }
        }
        let Some(x1) = x1 else {
            return Err(FindEntry("x1", record.to_string()));
        };
        let Some(x2) = x2 else {
            return Err(FindEntry("x2", record.to_string()));
        };
        let total_energy: N64 =
            event.outgoing_by_pid().iter().map(|(_, p)| p[0]).sum();
        let beam_energy = total_energy / (x1 + x2);
        let e1 = beam_energy * x1;
        let e2 = beam_energy * x2;
        event.add_outgoing(PID_X1, [e1, n64(0.0), n64(0.0), e1].into());
        event.add_outgoing(PID_X2, [e2, n64(0.0), n64(0.0), -e2].into());
        Ok(event.build())
    }
}

fn weight_start(line: &str) -> IResult<&str, &str> {
    let (rest, _) = (tag("<se"), space1, tag("w=\"")).parse(line)?;
    let (start, rest) = line.split_at(line.len() - rest.len());
    Ok((rest, start))
}

fn particle_start(line: &str) -> IResult<&str, &str> {
    preceded(multispace0, tag("<p")).parse(line)
}

fn particle_status(line: &str) -> IResult<&str, i32> {
    let (rest, parsed) = (space0, tag("id=\""), i32).parse(line)?;
    Ok((rest, parsed.2))
}

fn particle_id(line: &str) -> IResult<&str, ParticleID> {
    let (rest, id) = preceded(char(','), i32).parse(line)?;
    Ok((rest, ParticleID::new(id)))
}

fn particle_momentum(line: &str) -> IResult<&str, FourVector> {
    let (rest, p) = (
        space0,
        double,
        char(','),
        double,
        char(','),
        double,
        char(','),
        double,
        space0,
    )
        .parse(line)?;
    Ok((rest, [n64(p.1), n64(p.3), n64(p.5), n64(p.7)].into()))
}

fn reweight_start(line: &str) -> IResult<&str, &str> {
    recognize((
        multispace0,
        tag("<rw"),
        multispace1,
        tag("ch=\""),
        i32,
        char('"'),
        multispace0,
        char('>'),
        multispace0,
    )).parse(line)
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

fn double(input: &str) -> IResult<&str, f64> {
    nom::number::complete::double(input)
}

fn ws_comma(input: &str) -> IResult<&str, &str> {
    recognize(opt((multispace0, char(','), multispace0))).parse(input)
}
