use std::fmt::{self, Display};
use std::path::PathBuf;
use std::str::FromStr;

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

#[derive(Debug, Copy, Clone)]
pub(crate) enum Compression {
    Bzip2,
    Gzip,
    Lz4,
    Zstd,
}

#[derive(Debug, Clone)]
pub(crate) struct UnknownCompressionAlgorithm(String);

impl Display for UnknownCompressionAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown compression algorithm: {}", self.0)
    }
}

impl FromStr for Compression {
    type Err = UnknownCompressionAlgorithm;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "bzip2" | "bz2" => Ok(Self::Bzip2),
            "gzip" | "gz" => Ok(Self::Gzip),
            "lz4" => Ok(Self::Lz4),
            "zstd" | "zstandard" => Ok(Self::Zstd),
            _ => Err(UnknownCompressionAlgorithm(s.to_string())),
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

    /// Whether to dump selected cells of interest
    #[structopt(short = "d", long)]
    pub(crate) dumpcells: bool,

    #[structopt(short = "c", long, help = "Compress output file.\nPossible settings are 'bzip2', 'gzip', 'zstd', 'lz4'")]
    pub(crate) compression: Option<Compression>,

    /// Verbosity level
    #[structopt(
        short,
        long,
        default_value = "Info",
        help = "Verbosity level.\nPossible values with increasing amount of output are\n'off', 'error', 'warn', 'info', 'debug', 'trace'."
    )]
    pub(crate) loglevel: String,

    /// Input files
    #[structopt(name = "INFILES", parse(from_os_str))]
    pub(crate) infiles: Vec<PathBuf>,
}
