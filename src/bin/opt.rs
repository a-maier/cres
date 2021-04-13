use std::fmt::{self, Display};
use std::path::PathBuf;
use std::str::FromStr;

use cres::cell::Strategy;

use lazy_static::lazy_static;
use regex::Regex;
use structopt::StructOpt;

#[derive(Debug, Copy, Clone)]
pub(crate) enum JetAlgorithm {
    AntiKt,
    CambridgeAachen,
    Kt,
}

#[derive(Debug, Clone)]
pub(crate) struct UnknownJetAlgorithm(String);

impl Display for UnknownJetAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown jet algorithm: {}", self.0)
    }
}

impl FromStr for JetAlgorithm {
    type Err = UnknownJetAlgorithm;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anti_kt" | "antikt" | "anti-kt" => Ok(Self::AntiKt),
            "kt" => Ok(Self::Kt),
            "Cambridge/Aachen" | "Cambridge-Aachen" | "Cambridge_Aachen"
            | "cambridge/aachen" | "cambridge-aachen" | "cambridge_aachen" => {
                Ok(Self::CambridgeAachen)
            }
            _ => Err(UnknownJetAlgorithm(s.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnknownStrategy(String);

impl Display for UnknownStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown strategy: {}", self.0)
    }
}

fn parse_strategy(s: &str) -> Result<Strategy, UnknownStrategy> {
    use cres::cell::Strategy::*;
    match s {
        "Any" | "any" => Ok(Next),
        "MostNegative" | "most_negative" => Ok(MostNegative),
        "LeastNegative" | "least_negative" => Ok(LeastNegative),
        _ => Err(UnknownStrategy(s.to_string())),
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Compression {
    Bzip2,
    Gzip(u8),
    Lz4(u8),
    Zstd(u8),
}

#[derive(Debug, Clone)]
pub(crate) enum ParseCompressionErr {
    Algorithm(String),
    Level(String, String),
}

impl Display for ParseCompressionErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Algorithm(s) => write!(f, "Unknown compression algorithm: {}", s),
            Self::Level(algo,lvl) =>
                write!(f, "Level {} not supported for {} compression", lvl, algo)
        }
    }
}

lazy_static!{
    static ref COMPRESSION_RE: Regex = Regex::new(r#"^(?P<algo>[[:alpha:]]+)(?P<lvl>_\d+)?$"#).unwrap();
}

const GZIP_DEFAULT_LEVEL: u8 = 6;
const LZ4_DEFAULT_LEVEL: u8 = 0;
const ZSTD_DEFAULT_LEVEL: u8 = 0;

impl FromStr for Compression {
    type Err = ParseCompressionErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower_case = s.to_ascii_lowercase().to_string();
        let captures = COMPRESSION_RE.captures(&lower_case);
        let captures = if let Some(captures) = captures {
            captures
        } else {
            return Err(Self::Err::Algorithm(s.to_owned()))
        };
        let algo = &captures["algo"];
        let lvl_str = &captures.name("lvl");
        match algo {
            "bzip2" | "bz2" => {
                if let Some(lvl_str) = lvl_str {
                    Err(Self::Err::Level(algo.into(), lvl_str.as_str().to_owned()))
                } else {
                    Ok(Self::Bzip2)
                }
            },
            "gzip" | "gz" => {
                if let Some(lvl_str) = lvl_str {
                    match lvl_str.as_str()[1..].parse::<u8>() {
                        Ok(lvl) if lvl <= 9 => Ok(Self::Gzip(lvl)),
                        _ => Err(Self::Err::Level(algo.into(), lvl_str.as_str().to_owned()))
                    }
                } else {
                    Ok(Self::Gzip(GZIP_DEFAULT_LEVEL))
                }
            },
            "lz4" => {
                if let Some(lvl_str) = lvl_str {
                    match lvl_str.as_str()[1..].parse::<u8>() {
                        Ok(lvl) if lvl <= 16 => Ok(Self::Lz4(lvl)),
                        _ => Err(Self::Err::Level(algo.into(), lvl_str.as_str().to_owned()))
                    }
                } else {
                    Ok(Self::Lz4(LZ4_DEFAULT_LEVEL))
                }
            },
            "zstd" | "zstandard" => {
                if let Some(lvl_str) = lvl_str {
                    match lvl_str.as_str()[1..].parse::<u8>() {
                        Ok(lvl) if lvl <= 19 => Ok(Self::Zstd(lvl)),
                        _ => Err(Self::Err::Level(algo.into(), lvl_str.as_str().to_owned()))
                    }
                } else {
                    Ok(Self::Zstd(ZSTD_DEFAULT_LEVEL))
                }
            }
            _ => {
                Err(Self::Err::Algorithm(s.to_string()))
            },
        }
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

    #[structopt(short = "n", long, default_value = "1.", help = "Factor between cross section and sum of weights:
σ = weight_norm * Σ(weights)")]
    pub(crate) weight_norm: f64,

    /// Whether to dump selected cells of interest
    #[structopt(short = "d", long)]
    pub(crate) dumpcells: bool,

    #[structopt(short = "c", long, help = "Compress output file.
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
'off', 'error', 'warn', 'info', 'debug', 'trace'."
    )]
    pub(crate) loglevel: String,

    #[structopt(
        long, default_value = "least_negative",
        parse(try_from_str = parse_strategy),
        help = "Strategy for choosing cell seeds. Possible values are
'least_negative': event with negative weight closest to zero,
'most_negative' event with the lowest weight,
'any': no additional requirements beyond a negative weight.\n"
    )]
    pub(crate) strategy: Strategy,

    /// Input files
    #[structopt(name = "INFILES", parse(from_os_str))]
    pub(crate) infiles: Vec<PathBuf>,
}
