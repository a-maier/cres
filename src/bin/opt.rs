use std::path::PathBuf;

use cres::compression::Compression;
use cres::hepmc2::converter::JetAlgorithm;
use cres::resampler::{Strategy, UnknownStrategy};

use lazy_static::lazy_static;
use regex::Regex;
use structopt::StructOpt;
use thiserror::Error;

fn parse_strategy(s: &str) -> Result<Strategy, UnknownStrategy> {
    use Strategy::*;
    match s {
        "Any" | "any" => Ok(Next),
        "MostNegative" | "most_negative" => Ok(MostNegative),
        "LeastNegative" | "least_negative" => Ok(LeastNegative),
        _ => Err(UnknownStrategy(s.to_string())),
    }
}

#[derive(Debug, Clone, Error)]
pub(crate) enum ParseCompressionErr {
    #[error("Unknown compression algorithm: {0}")]
    UnknownAlgorithm(String),
    #[error("Level {0} not supported for {1} compression")]
    UnsupportedLevel(String, String),
}

lazy_static!{
    static ref COMPRESSION_RE: Regex = Regex::new(r#"^(?P<algo>[[:alpha:]]+)(?P<lvl>_\d+)?$"#).unwrap();
}

const GZIP_DEFAULT_LEVEL: u8 = 6;
const LZ4_DEFAULT_LEVEL: u8 = 0;
const ZSTD_DEFAULT_LEVEL: u8 = 0;

fn parse_compr(s: &str) -> Result<Compression, ParseCompressionErr> {
    use Compression::*;
    use ParseCompressionErr::*;

    let lower_case = s.to_ascii_lowercase();
    let captures = COMPRESSION_RE.captures(&lower_case);
    let captures = if let Some(captures) = captures {
        captures
    } else {
        return Err(UnknownAlgorithm(s.to_owned()))
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
        },
        "gzip" | "gz" => {
            if let Some(lvl_str) = lvl_str {
                match lvl_str.as_str()[1..].parse::<u8>() {
                    Ok(lvl) if lvl <= 9 => Ok(Gzip(lvl)),
                    _ => Err(UnsupportedLevel(algo.into(), lvl_str.as_str().to_owned()))
                }
            } else {
                Ok(Gzip(GZIP_DEFAULT_LEVEL))
            }
        },
        "lz4" => {
            if let Some(lvl_str) = lvl_str {
                match lvl_str.as_str()[1..].parse::<u8>() {
                    Ok(lvl) if lvl <= 16 => Ok(Lz4(lvl)),
                    _ => Err(UnsupportedLevel(algo.into(), lvl_str.as_str().to_owned()))
                }
            } else {
                Ok(Lz4(LZ4_DEFAULT_LEVEL))
            }
        },
        "zstd" | "zstandard" => {
            if let Some(lvl_str) = lvl_str {
                match lvl_str.as_str()[1..].parse::<u8>() {
                    Ok(lvl) if lvl <= 19 => Ok(Zstd(lvl)),
                    _ => Err(UnsupportedLevel(algo.into(), lvl_str.as_str().to_owned()))
                }
            } else {
                Ok(Zstd(ZSTD_DEFAULT_LEVEL))
            }
        }
        _ => {
            Err(UnknownAlgorithm (s.to_string()))
        },
    }
}

#[derive(Debug, Copy, Clone, StructOpt)]
pub(crate) struct JetDefinition {
    /// Jet algorithm
    #[structopt(
        short = "a",
        long,
        help = "Jet algorithm.\nPossible settings are 'anti-kt', 'kt', 'Cambridge-Aachen'"
    )]
    pub jetalgorithm: JetAlgorithm,
    /// Jet radius parameter
    #[structopt(short = "R", long)]
    pub jetradius: f64,
    #[structopt(short = "p", long)]
    /// Minimum jet transverse momentum
    pub jetpt: f64,
}

impl std::convert::From<JetDefinition> for cres::hepmc2::converter::JetDefinition {
    fn from(j: JetDefinition) -> Self {
        Self {
            jetalgorithm: j.jetalgorithm,
            jetradius: j.jetradius,
            jetpt: j.jetpt,
        }
    }
}

#[derive(Debug, Copy, Clone, StructOpt)]
pub(crate) struct UnweightOpt {
    /// Weight below which events are unweighted
    #[structopt(short = "w", long, default_value = "0.")]
    pub(crate) minweight: f64,

    /// Random number generator seed for unweighting
    #[structopt(short, long, default_value = "0")]
    pub(crate) seed: u64,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "cres", about = "Make event weights positive")]
pub(crate) struct Opt {
    /// Output file
    #[structopt(long, short, parse(from_os_str))]
    pub(crate) outfile: PathBuf,

    #[structopt(flatten)]
    pub(crate) jet_def: JetDefinition,

    #[structopt(flatten)]
    pub(crate) unweight: UnweightOpt,

    ///
    #[structopt(long, default_value = "0.", help = "Weight of transverse momentum
when calculating particle momentum distances.\n")]
    pub(crate) ptweight: f64,

    #[structopt(short = "n", long, default_value = "1.", help = "Factor between cross section and sum of weights:
σ = weight_norm * Σ(weights)")]
    pub(crate) weight_norm: f64,

    /// Whether to dump selected cells of interest
    #[structopt(short = "d", long)]
    pub(crate) dumpcells: bool,

    #[structopt(short = "c", long, parse(try_from_str = parse_compr),
                help = "Compress output file.
Possible settings are 'bzip2', 'gzip', 'zstd', 'lz4'
Compression levels can be set with algorithm_level e.g. 'zstd_5'.
Maximum levels are 'gzip_9', 'zstd_19', 'lz4_16'.")]
    pub(crate) compression: Option<Compression>,

    /// Verbosity level
    #[structopt(
        short,
        long,
        default_value = "Info",
        help = "Verbosity level.
Possible values with increasing amount of output are
'off', 'error', 'warn', 'info', 'debug', 'trace'.\n"
    )]
    pub(crate) loglevel: String,

    #[structopt(
        long, default_value = "most_negative",
        parse(try_from_str = parse_strategy),
        help = "Strategy for choosing cell seeds. Possible values are
'least_negative': event with negative weight closest to zero,
'most_negative' event with the lowest weight,
'any': no additional requirements beyond a negative weight.\n"
    )]
    pub(crate) strategy: Strategy,

    #[structopt(long,
        help = "Maximum cell size. Limiting the cell size can cause
left-over negative-weight events."
    )]
    pub(crate) max_cell_size: Option<f64>,

    /// Input files
    #[structopt(name = "INFILES", parse(from_os_str))]
    pub(crate) infiles: Vec<PathBuf>,
}
