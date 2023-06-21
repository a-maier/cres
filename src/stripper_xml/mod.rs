pub mod reader;
use quick_xml::DeError;
pub use reader::Reader;

use std::{path::{PathBuf, Path}, io::BufReader, str::Utf8Error, num::ParseIntError, borrow::Cow};

use noisy_float::prelude::*;
use stripper_xml::normalization::Normalization;
use thiserror::Error;

use crate::file::File;

pub fn extract_xml_info(
    path: &Path,
    buf: &[u8]
) -> Result<XMLTag, XMLError> {
    use quick_xml::events::Event;
    use XMLError::*;

    let mut reader = quick_xml::Reader::from_reader(buf);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"Normalization" => {
                        let file = File::open(path)?;
                        let input = BufReader::new(file);
                        let norm: Normalization = quick_xml::de::from_reader(input)?;
                        return Ok(XMLTag::Normalization {
                            name: norm.contribution.name,
                            scale: n64(norm.contribution.xsection.0[0])
                        });
                    },
                    b"Eventrecord" => {
                        let mut name = None;
                        let mut nevents = None;
                        let attributes = e.attributes()
                            .filter_map(|a| match a {
                                Ok(a) => Some(a),
                                Err(_) => None,
                            });
                        for attr in attributes {
                            match attr.key.0 {
                                b"nevents" => nevents = Some(parse_u64(attr.value.as_ref())?),
                                b"name" => name = Some(to_string(attr.value)?),
                                _ => { }
                            }
                        }
                        let Some(name) = name else {
                            return Err(NoEventrecordAttr(path.to_owned(), "name"));
                        };
                        let Some(nevents) = nevents else {
                            return Err(NoEventrecordAttr(path.to_owned(), "nevents"));
                        };
                        return Ok(XMLTag::Eventrecord { name, nevents });
                    },
                    _name => return Err(BadTag(path.to_owned()))
                }
            },
            Ok(Event::Decl(_) | Event::Text(_)) => { } // ignore,
            _ => return Err(NoTag(path.into()))
        }
    }
}

fn to_string(value: Cow<[u8]>) -> Result<String, XMLError> {
    match value {
        Cow::Borrowed(s) => Ok(std::str::from_utf8(s)?.to_owned()),
        Cow::Owned(s) => Ok(String::from_utf8(s).map_err(|e| e.utf8_error())?),
    }
}

fn parse_u64(num: &[u8]) -> Result<u64, XMLError> {
    let num: &str = std::str::from_utf8(num)?;
    let num = num.parse()?;
    Ok(num)
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum XMLTag {
    Normalization{ name: String, scale: N64 },
    Eventrecord{ name: String, nevents: u64 },
}

#[derive(Debug, Error)]
pub enum XMLError {
    #[error("Failed to open file: {0}")]
    FileOpen(#[from] std::io::Error),
    #[error("File `{0}` does not start with an XML tag")]
    NoTag(PathBuf),
    #[error("File `{0}` does not starts with an unsupported XML tag")]
    BadTag(PathBuf),
    #[error("XML tag `Eventrecord` in file `{0}` does not have a `{1}` attribute")]
    NoEventrecordAttr(PathBuf, &'static str),
    #[error("Failed to deserialise `Normalization`: {0}")]
    NormalizationDeser(#[from] DeError),
    #[error("Utf8 error: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("Failed to parse integer: {0}")]
    ParseInt(#[from] ParseIntError),
}
