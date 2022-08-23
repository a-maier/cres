use std::fmt::{self, Display};
use std::path::PathBuf;
use std::str::FromStr;

use cres::compression::Compression;
use cres::cluster::JetAlgorithm;
use cres::seeds::Strategy;

use clap::{ArgEnum, Parser};
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;
use strum::{Display, EnumString};

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
pub struct UnknownStrategy(pub String);

impl Display for UnknownStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown strategy: {}", self.0)
    }
}

#[derive(Debug, Clone, Error)]
pub(crate) enum ParseCompressionErr {
    #[error("Unknown compression algorithm: {0}")]
    UnknownAlgorithm(String),
    #[error("Level {0} not supported for {1} compression")]
    UnsupportedLevel(String, String),
}

lazy_static! {
    static ref COMPRESSION_RE: Regex =
        Regex::new(r#"^(?P<algo>[[:alpha:]]+)(?P<lvl>_\d+)?$"#).unwrap();
}

const GZIP_DEFAULT_LEVEL: u8 = 6;
const LZ4_DEFAULT_LEVEL: u8 = 0;
const ZSTD_DEFAULT_LEVEL: u8 = 0;

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
    /// Minimum jet transverse momentum.
    pub jetpt: f64,
}

impl std::convert::From<JetDefinition> for cres::cluster::JetDefinition {
    fn from(j: JetDefinition) -> Self {
        Self {
            algorithm: j.jetalgorithm,
            radius: j.jetradius,
            min_pt: j.jetpt,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
pub(crate) enum Search {
    Tree,
    Naive
}

#[derive(Debug, Copy, Clone, Parser)]
pub(crate) struct UnweightOpt {
    /// Weight below which events are unweighted.
    #[clap(short = 'w', long, default_value = "0.")]
    pub(crate) minweight: f64,

    /// Random number generator seed for unweighting.
    #[clap(long, default_value = "0")]
    pub(crate) seed: u64,
}

#[derive(Debug, Display, Default, Copy, Clone, ArgEnum, EnumString)]
#[clap(rename_all = "lower")]
pub(crate) enum FileFormat {
    #[default]
    HepMC2,
    #[cfg(feature = "ntuple")]
    Root
}

#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub(crate) struct Opt {
    /// Output file.
    #[clap(long, short, parse(from_os_str))]
    pub(crate) outfile: PathBuf,

    #[clap(flatten)]
    pub(crate) jet_def: JetDefinition,

    #[clap(flatten)]
    pub(crate) unweight: UnweightOpt,

    /// Weight of transverse momentum when calculating particle momentum distances.
    #[clap(long, default_value = "0.")]
    pub(crate) ptweight: f64,

    /// Whether to dump selected cells of interest.
    #[clap(short = 'd', long)]
    pub(crate) dumpcells: bool,

    #[clap(short = 'c', long, parse(try_from_str = parse_compr),
                help = "Compress output file.
Possible settings are 'bzip2', 'gzip', 'zstd', 'lz4'.
Compression levels can be set with algorithm_level e.g. 'zstd_5'.
Maximum levels are 'gzip_9', 'zstd_19', 'lz4_16'.")]
    pub(crate) compression: Option<Compression>,

    /// Output format.
    #[clap(arg_enum, long, default_value_t)]
    pub(crate) outformat: FileFormat,

    /// Verbosity level
    #[clap(
        short,
        long,
        default_value = "Info",
        help = "Verbosity level.
Possible values with increasing amount of output are
'off', 'error', 'warn', 'info', 'debug', 'trace'.\n"
    )]
    pub(crate) loglevel: String,

    #[clap(long, default_value = "1", validator = is_power_of_two,
        help = "Number of partitions.

The input event sample is split into the given number of partitions,
which has to be a power of two. Each partition is resampled
separately in parallel."
    )]
    pub(crate) partitions: u32,

    /// Algorithm for finding nearest-neighbour events.
    ///
    /// Note that the 'tree' search is not parallelised. To benefit from
    /// parallelisation use the `--partitions` options in addition.
    #[clap(arg_enum, short, long, default_value = "tree")]
    pub(crate) search: Search,

    #[clap(
        long, default_value = "most_negative",
        parse(try_from_str = parse_strategy),
        help = "Strategy for choosing cell seeds. Possible values are
'least_negative': event with negative weight closest to zero,
'most_negative' event with the lowest weight,
'any': no additional requirements beyond a negative weight.\n"
    )]
    pub(crate) strategy: Strategy,

    #[clap(short, long, default_value_t,
    help ="Number of threads.

If set to 0, a default number of threads is chosen.
The default can be set with the `RAYON_NUM_THREADS` environment
variable."
    )]
    pub(crate) threads: usize,

    /// Maximum cell size.
    ///
    /// Limiting the cell size ensures that event weights are only
    /// redistributed between events that are sufficiently similar.
    /// The downside is that not all negative weights may be cancelled.
    #[clap(long)]
    pub(crate) max_cell_size: Option<f64>,

    /// Input files
    #[clap(name = "INFILES", parse(from_os_str))]
    pub(crate) infiles: Vec<PathBuf>,
}

pub(crate) fn is_power_of_two(s: &str) -> Result<(), String> {
    match u32::from_str(s) {
        Ok(n) => if n.is_power_of_two() {
            Ok(())
        } else {
            Err("has to be a power of two".to_string())
        }
        Err(err) => Err(err.to_string())
    }
}
