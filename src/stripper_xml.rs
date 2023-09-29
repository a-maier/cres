use std::{path::{PathBuf, Path}, io::{BufRead, Read, Write, BufWriter, BufReader}, collections::HashMap, fs::File, borrow::Cow, str::Utf8Error, num::ParseIntError, fmt::{Display, self}, string::FromUtf8Error};

use audec::auto_decompress;
use log::{debug, trace};
use noisy_float::prelude::*;
use nom::{sequence::{tuple, preceded}, character::complete::{char, i32, space0, space1, multispace0}, number::complete::double, IResult, bytes::complete::tag};
use particle_id::ParticleID;
use stripper_xml::normalization::Normalization;
use thiserror::Error;

use crate::{compression::{Compression, compress_writer}, traits::{Rewind, UpdateWeights}, storage::{StorageError, EventRecord, CreateError, Converter}, util::trim_ascii_start, event::{Event, EventBuilder, Weights}, four_vector::FourVector};

/// Storage backed by (potentially compressed) Les Houches Event Files
pub struct FileStorage {
    source_path: PathBuf,
    source: Box<dyn BufRead>,
    _sink_path: PathBuf,
    sink: Box<dyn Write>,
    _weight_names: Vec<String>,
    weight_scale: f64,
    rem_subevents: usize,
}

impl FileStorage {
    /// Construct STRIPPER XML event storage
    ///
    /// Construct a storage backed by the given (potentially compressed)
    /// STRIPPER XMLs file with the given information for
    /// channel-specific scale factors
    pub fn try_new( // TODO: use builder pattern instead?
        source_path: PathBuf,
        sink_path: PathBuf,
        compression: Option<Compression>,
        _weight_names: Vec<String>,
        scaling: &HashMap<String, f64>,
    ) -> Result<Self, CreateError> {
        use CreateError::*;

        let (header, source) = init_source(&source_path)?;
        let outfile = File::create(&sink_path)?;
        let sink = BufWriter::new(outfile);
        let mut sink = compress_writer(sink, compression)?;
        sink.write_all(&header)?;
        let header_info = extract_xml_info(header.as_slice())
            .map_err(|e| XMLError(source_path.clone(), e))?;
        let XMLTag::Eventrecord {
            alpha_s_power: _,
            name,
            nevents: _,
            nsubevents
        } = header_info else {
            return Err(XMLError(
                source_path,
                Error::BadTag(header_info.to_string())
            ))
        };

        let Some(weight_scale) = scaling.get(&name).copied() else {
            return Err(XMLError(
                source_path,
                Error::MissingScaling(name)
            ))
        };

        Ok(FileStorage {
            source_path,
            source,
            _sink_path: sink_path,
            sink,
            _weight_names,
            weight_scale,
            rem_subevents: nsubevents as usize,
        })
    }

    fn read_record(&mut self) -> Option<Result<String, Error>> {
        let mut record = b"<se".to_vec();
        loop {
            match self.source.read_until(b'e', &mut record) {
                Ok(0) => if record.len() > 3 {
                    break;
                } else {
                    return None;
                },
                Ok(_) => if record.ends_with(b"<se") {
                    record.truncate(record.len() - 3);
                    break;
                },
                Err(err) => return Some(Err(err.into())),
            }
        }

        let record = match String::from_utf8(record) {
            Ok(record) => record,
            Err(err) => return Some(Err(err.into())),
        };

        assert!(record.starts_with("<se"));

        trace!("Read STRIPPER XML record:\n{record}");

        self.rem_subevents -= 1;
        Some(Ok(record))
    }

    fn rescale_weight(&self, record: &mut String) -> Result<(), Error> {
        let (rest, start) = weight_start(record.as_str())?;
        let (rest, weight) = double(rest)?;
        let start = start.len();
        let end = record.len() - rest.len();
        record.replace_range(start..end, &(self.weight_scale * weight).to_string());
        trace!("rescaled weight: {weight} -> {}", self.weight_scale * weight);
        Ok(())
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
                Ok(mut record) => {
                    // TODO: might be better to do this in the Converter, but it
                    // doesn't have the information
                    if let Err(err) = self.rescale_weight(&mut record) {
                        return Err(err.into());
                    } else {
                        Ok(EventRecord::StripperXml(record))
                    }
                },
                Err(err) => Err(err.into()),
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.rem_subevents, Some(self.rem_subevents))
    }
}

pub(crate) fn extract_scaling<I, P, Q>(
    paths: I,
) -> Result<(Vec<(PathBuf, Q)>, HashMap<String, f64>), CreateError>
where
    I: IntoIterator<Item = (P, Q)>,
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let mut event_files = Vec::new();
    let mut rescale: HashMap<_, (f64, u64)> = HashMap::new();
    for (path, out) in paths {
        let path = path.as_ref();
        let file = File::open(path)?;
        let mut r = auto_decompress(BufReader::new(file));
        if let Ok(buf) = r.fill_buf() {
            let buf = trim_ascii_start(buf);
            if buf.starts_with(b"<?xml") {
                debug!("extracting scaling information from {path:?}");
                let tag = extract_xml_info(r).map_err(|err| {
                    CreateError::XMLError(path.to_owned(), err)
                })?;
                match tag {
                    XMLTag::Normalization { name, scale } => {
                        let entry = rescale.entry(name).or_default();
                        entry.0 = scale;
                        // don't add to vec of event files
                    }
                    XMLTag::Eventrecord { name, nevents, .. } => {
                        let entry = rescale.entry(name).or_insert((-1., 0));
                        entry.1 += nevents;
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
        .map(|(name, (scale, nevents))| (name, scale / (nevents as f64)))
        .collect();
    Ok((event_files, rescale))
}

pub(crate) fn extract_xml_info(r: impl BufRead) -> Result<XMLTag, Error> {
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
                            quick_xml::de::from_reader(all)?;
                        return Ok(XMLTag::Normalization {
                            name: norm.contribution.name,
                            scale: norm.contribution.xsection.0[0],
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
                            match attr.key.0 {
                                b"nevents" => {
                                    nevents =
                                        Some(parse_u64(attr.value.as_ref())?)
                                }
                                b"nsubevents" => {
                                    nsubevents =
                                        Some(parse_u64(attr.value.as_ref())?)
                                }
                                b"name" => name = Some(to_string(attr.value)?),
                                b"as" => {
                                    alpha_s_power =
                                        Some(parse_u64(attr.value.as_ref())?)
                                }
                                _ => {}
                            }
                        }
                        let Some(name) = name else {
                            return Err(NoEventrecordAttr("name"));
                        };
                        let Some(nsubevents) = nsubevents else {
                            return Err(NoEventrecordAttr("nsubevents"));
                        };
                        let Some(nevents) = nevents else {
                            return Err(NoEventrecordAttr("nevents"));
                        };
                        let Some(alpha_s_power) = alpha_s_power else {
                            return Err(NoEventrecordAttr("as"));
                        };
                        return Ok(XMLTag::Eventrecord {
                            alpha_s_power,
                            name,
                            nevents,
                            nsubevents,
                        });
                    }
                    name => {
                        let name = std::str::from_utf8(name)?;
                        return Err(BadTag(name.to_owned()));
                    }
                }
            }
            Ok(Event::Decl(_) | Event::Text(_)) => {} // ignore,
            _ => return Err(NoTag),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub(crate) enum XMLTag {
    Normalization {
        name: String,
        scale: f64,
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
            XMLTag::Normalization { name, scale } => write!(
                f, r#"<Normalization name="{name}" scale="{scale}">"#
            ),
            XMLTag::Eventrecord { alpha_s_power, name, nevents, nsubevents } => write!(
                f, r#"<Eventrecord nevents="{nevents}" nsubevents="{nsubevents}" as="{alpha_s_power}" name="{name}"">"#
            ),
        }
    }
}


fn to_string(value: Cow<[u8]>) -> Result<String, Error> {
    match value {
        Cow::Borrowed(s) => Ok(std::str::from_utf8(s)?.to_owned()),
        Cow::Owned(s) => Ok(String::from_utf8(s).map_err(|e| e.utf8_error())?),
    }
}

fn parse_u64(num: &[u8]) -> Result<u64, Error> {
    let num: &str = std::str::from_utf8(num)?;
    let num = num.parse()?;
    Ok(num)
}

/// STRIPPER XML Error
#[derive(Debug, Error)]
pub enum Error {
    /// Error opening a file
    #[error("Failed to open file: {0}")]
    FileOpen(#[from] std::io::Error),
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
    /// Deserialisation error for [stripper_xml::Normalization]
    #[error("Failed to deserialise `Normalization`: {0}")]
    NormalizationDeser(#[from] quick_xml::DeError),
    /// UTF8 error
    #[error("UTF8 error: {0}")]
    Utf8(#[from] Utf8Error),
    /// UTF8 error
    #[error("UTF8 error: {0}")]
    FromUtf8(#[from] FromUtf8Error),
    /// Error parsing an integer
    #[error("Failed to parse integer: {0}")]
    ParseInt(#[from] ParseIntError),
    /// Parsing error
    #[error("Parsing error: {0}")]
    ParseError(String),
    /// Unclosed tag
    #[error("Tag {0} is not closed in {1}")]
    UnclosedTag(String, String),
}

impl From<nom::Err<nom::error::Error<&str>>> for Error {
    fn from(source: nom::Err<nom::error::Error<&str>>) -> Self {
        Self::ParseError(source.to_string())
    }
}

fn init_source(source: impl AsRef<Path>) -> Result<(Vec<u8>, Box<dyn BufRead>), std::io::Error> {
    let source = File::open(source)?;
    let mut buf = auto_decompress(BufReader::new(source));

    // read until start of first event
    let mut header = Vec::new();
    while !header.ends_with(b"<se") {
        if buf.read_until(b'e', &mut header)? == 0 {
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
    fn parse_stripper_xml(&self, record: &str) -> Result<Event, Self::Error>;
}

impl StripperXmlParser for Converter {
    type Error = Error;

    fn parse_stripper_xml(&self, record: &str) -> Result<Event, Self::Error> {
        const STATUS_OUTGOING: i32 = 0;

        let mut event = EventBuilder::new();

        let (rest, _start) = weight_start(record)?;
        let (rest, weight) = double(rest)?;
        // TODO: it might be best to do the weight rescaling here, but
        // the Converter doesn't have that information
        event.add_weight(n64(weight));

        let Some(tag_end) = rest.find('>') else {
            return Err(Error::ParseError(
                format!("Failed to find end of <se> tag in {record}")
            ))
        };
        let mut rest = &rest[(tag_end + 1)..];

        while let Ok((r, _)) = particle_start(rest) {
            let (r, status) = particle_status(r)?;
            if status != STATUS_OUTGOING {
                let Some(particle_end) = r.find("</p>") else {
                    return Err(Error::UnclosedTag("<p".to_string(), rest.to_string()))
                };
                rest = &r[(particle_end + "</p>".len())..];
                continue;
            }

            let (r, pid) = particle_id(r)?;
            let (r, _) = tag("\">")(r)?;
            let (r, p) = particle_momentum(&r[1..])?;
            event.add_outgoing(pid, p);
            (rest, _) = tag("</p>")(r)?;
        }

        // TODO: multiple weights

        Ok(event.build())
    }
}

fn weight_start(line: &str)-> IResult<&str, &str> {
    let (rest, _) = tuple((tag("<se"), space1, tag("w=\"")))(line)?;
    let (start, rest) = line.split_at(line.len() - rest.len());
    Ok((rest, start))
}

fn particle_start(line: &str) -> IResult<&str, &str> {
    preceded(multispace0, tag("<p"))(line)
}

fn particle_status(line: &str) -> IResult<&str, i32> {
    let (rest, parsed) = tuple((space0, tag("id=\""), i32))(line)?;
    Ok((rest, parsed.2))
}

fn particle_id(line: &str) -> IResult<&str, ParticleID> {
    let (rest, id) = preceded(char(','), i32)(line)?;
    Ok((rest, ParticleID::new(id)))
}

fn particle_momentum(line: &str) -> IResult<&str, FourVector> {
    let (rest, p) = tuple((
        space0, double, char(','), double, char(','), double, char(','), double, space0
    ))(line)?;
    Ok((rest, [n64(p.1), n64(p.3), n64(p.5), n64(p.7)].into()))
}

impl UpdateWeights for FileStorage {
    type Error = StorageError;

    fn update_all_weights(&mut self, weights: &[Weights]) -> Result<usize, Self::Error> {
        self.rewind()?;
        for (n, weight) in weights.iter().enumerate() {
            if !self.update_next_weights(weight)? {
                return Ok(n)
            }
        }
        Ok(weights.len())
    }

    fn update_next_weights(&mut self, weights: &Weights) -> Result<bool, Self::Error> {
        let Some(record) = self.read_record() else {
            return Ok(false)
        };
        let mut record = record?;

        #[cfg(feature = "multiweight")]
        let weight = weights[0];
        #[cfg(not(feature = "multiweight"))]
        let weight = weights;

        let weight = weight / self.weight_scale;

        // TODO: code duplication with `rescale_weight`
        let (rest, start) = weight_start(record.as_str())
            .map_err(Error::from)?;
        let (rest, old_weight) = double(rest)
            .map_err(Error::from)?;
        let start = start.len();
        let end = record.len() - rest.len();
        record.replace_range(start..end, &weight.to_string());
        trace!("replaced weight: {old_weight} -> {weight}");

        #[cfg(feature = "multiweight")]
        if weights.len() > 1 {
            unimplemented!("Multiple weights in STRIPPER XML format")
        }
        self.sink.write_all(record.as_bytes())?;
        Ok(true)
    }
}
