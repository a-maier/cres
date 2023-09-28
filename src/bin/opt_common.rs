use std::convert::From;

use clap::{Parser, ValueEnum};
use cres::cluster::JetAlgorithm;
use cres::compression::Compression;
use cres::writer::OutputFormat;

use lazy_static::lazy_static;
use regex::Regex;
use strum::{Display, EnumString};
use thiserror::Error;

const GZIP_DEFAULT_LEVEL: u8 = 6;
const LZ4_DEFAULT_LEVEL: u8 = 0;
const ZSTD_DEFAULT_LEVEL: u8 = 0;

lazy_static! {
    static ref COMPRESSION_RE: Regex =
        Regex::new(r"^(?P<algo>[[:alnum:]]+)(?P<lvl>_\d+)?$").unwrap();
}

pub(crate) fn parse_compr(s: &str) -> Result<Compression, ParseCompressionErr> {
    use Compression::*;
    use ParseCompressionErr::*;

    let lower_case = s.to_ascii_lowercase();
    let captures = COMPRESSION_RE.captures(&lower_case);
    let captures = if let Some(captures) = captures {
        captures
    } else {
        return Err(UnknownAlgorithm(s.to_owned()));
    };
    let algo = &captures["algo"];
    let lvl_str = &captures.name("lvl");
    match algo {
        "bzip2" | "bz2" => {
            if let Some(lvl_str) = lvl_str {
                Err(UnsupportedLevel(algo.into(), lvl_str.as_str().to_owned()))
            } else {
                Ok(Bzip2)
            }
        }
        "gzip" | "gz" => {
            if let Some(lvl_str) = lvl_str {
                match lvl_str.as_str()[1..].parse::<u8>() {
                    Ok(lvl) if lvl <= 9 => Ok(Gzip(lvl)),
                    _ => Err(UnsupportedLevel(
                        algo.into(),
                        lvl_str.as_str().to_owned(),
                    )),
                }
            } else {
                Ok(Gzip(GZIP_DEFAULT_LEVEL))
            }
        }
        "lz4" => {
            if let Some(lvl_str) = lvl_str {
                match lvl_str.as_str()[1..].parse::<u8>() {
                    Ok(lvl) if lvl <= 16 => Ok(Lz4(lvl)),
                    _ => Err(UnsupportedLevel(
                        algo.into(),
                        lvl_str.as_str().to_owned(),
                    )),
                }
            } else {
                Ok(Lz4(LZ4_DEFAULT_LEVEL))
            }
        }
        "zstd" | "zstandard" => {
            if let Some(lvl_str) = lvl_str {
                match lvl_str.as_str()[1..].parse::<u8>() {
                    Ok(lvl) if lvl <= 19 => Ok(Zstd(lvl)),
                    _ => Err(UnsupportedLevel(
                        algo.into(),
                        lvl_str.as_str().to_owned(),
                    )),
                }
            } else {
                Ok(Zstd(ZSTD_DEFAULT_LEVEL))
            }
        }
        _ => Err(UnknownAlgorithm(s.to_string())),
    }
}

#[derive(Debug, Copy, Clone, Parser)]
pub(crate) struct JetDefinition {
    /// Jet algorithm.
    #[clap(
        short = 'a',
        long,
        help = "Jet algorithm.\nPossible settings are 'anti-kt', 'kt', 'Cambridge-Aachen'."
    )]
    pub jetalgorithm: JetAlgorithm,
    /// Jet radius parameter.
    #[clap(short = 'R', long)]
    pub jetradius: f64,
    #[clap(short = 'p', long)]
    /// Minimum jet transverse momentum in GeV.
    pub jetpt: f64,
}

impl From<JetDefinition> for cres::cluster::JetDefinition {
    fn from(j: JetDefinition) -> Self {
        Self {
            algorithm: j.jetalgorithm,
            radius: j.jetradius,
            min_pt: j.jetpt,
        }
    }
}

#[derive(Debug, Copy, Clone, Parser)]
pub(crate) struct LeptonDefinition {
    /// Lepton dressing algorithm.
    #[clap(
        long,
        help = "Lepton dressing algorithm.\nPossible settings are 'anti-kt', 'kt', 'Cambridge-Aachen'."
    )]
    pub leptonalgorithm: Option<JetAlgorithm>,
    /// Lepton radius parameter.
    #[clap(long)]
    pub leptonradius: Option<f64>,
    #[clap(long)]
    /// Minimum lepton transverse momentum in GeV.
    pub leptonpt: Option<f64>,
}

impl From<LeptonDefinition> for cres::cluster::JetDefinition {
    fn from(l: LeptonDefinition) -> Self {
        Self {
            algorithm: l.leptonalgorithm.unwrap(),
            radius: l.leptonradius.unwrap(),
            min_pt: l.leptonpt.unwrap(),
        }
    }
}

#[derive(Debug, Copy, Clone, Parser)]
pub(crate) struct PhotonDefinition {
    /// Minimum fraction of photon transverse energy in GeV.
    #[clap(long)]
    pub photonefrac: Option<f64>,
    /// Photon radius parameter.
    #[clap(long)]
    pub photonradius: Option<f64>,
    /// Minimum photon transverse momentum in GeV.
    #[clap(long)]
    pub photonpt: Option<f64>,
}

impl From<PhotonDefinition> for cres::cluster::PhotonDefinition {
    fn from(j: PhotonDefinition) -> Self {
        Self {
            min_e_fraction: j.photonefrac.unwrap(),
            radius: j.photonradius.unwrap(),
            min_pt: j.photonpt.unwrap(),
        }
    }
}

#[derive(Debug, Clone, Error)]
pub(crate) enum ParseCompressionErr {
    #[error("Unknown compression algorithm: {0}")]
    UnknownAlgorithm(String),
    #[error("Level {0} not supported for {1} compression")]
    UnsupportedLevel(String, String),
}

#[derive(Debug, Display, Default, Copy, Clone, ValueEnum, EnumString)]
#[clap(rename_all = "lower")]
pub(crate) enum FileFormat {
    #[default]
    HepMC2,
    #[cfg(feature = "lhef")]
    Lhef,
    #[cfg(feature = "ntuple")]
    Root,
    #[cfg(feature = "stripper-xml")]
    #[clap(name = "stripper-xml")]
    StripperXml,
}

impl From<FileFormat> for OutputFormat {
    fn from(source: FileFormat) -> Self {
        match source {
            FileFormat::HepMC2 => OutputFormat::HepMC2,
            #[cfg(feature = "lhef")]
            FileFormat::Lhef => OutputFormat::Lhef,
            #[cfg(feature = "ntuple")]
            FileFormat::Root => OutputFormat::Root,
            #[cfg(feature = "stripper-xml")]
            FileFormat::StripperXml => OutputFormat::StripperXml,
        }
    }
}
