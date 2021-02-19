use std::path::PathBuf;
use std::str::FromStr;
use std::fmt::{self, Display};

use structopt::StructOpt;

#[derive(Debug, Copy, Clone)]
pub(crate) struct JetDefinition {
    pub algo: JetAlgorithm,
    pub r: f64,
    pub minpt: f64
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum JetAlgorithm {
    AntiKt,
    CambridgeAachen,
    Kt,
}

#[derive(Debug, Clone)]
pub(crate) struct UnknownAlgorithm (String);

impl Display for UnknownAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown jet algorithm: {}", self.0)
    }
}

impl FromStr for JetAlgorithm {
    type Err = UnknownAlgorithm;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anti_kt" | "antikt" | "anti-kt" => Ok(Self::AntiKt),
            "kt" => Ok(Self::Kt),
            "Cambridge/Aachen"
                | "Cambridge-Aachen"
                | "Cambridge_Aachen"
                | "cambridge/aachen"
                | "cambridge-aachen"
                | "cambridge_aachen"
                => Ok(Self::CambridgeAachen),
            _ => Err(UnknownAlgorithm(s.to_string()))
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "cres", about = "Make event weights positive")]
pub(crate) struct Opt {
    /// Output file
    #[structopt(long, short, parse(from_os_str))]
    pub(crate) outfile: PathBuf,

    /// Jet algorithm
    #[structopt(short = "a", long, help = "Jet algorithm.\nPossible settings are 'anti-kt', 'kt', 'Cambridge-Aachen'")]
    pub(crate) jetalgorithm: JetAlgorithm,

    /// Jet radius parameter
    #[structopt(short = "R", long)]
    pub(crate) jetradius: f64,

    /// Minimum jet transverse momentum
    #[structopt(short = "p", long)]
    pub(crate) jetpt: f64,

    /// Weight below which events are unweighted
    #[structopt(short = "w", long, default_value = "0.")]
    pub(crate) minweight: f64,

    /// Random number generator seed for unweighting
    #[structopt(short, long, default_value = "0")]
    pub(crate) seed: u64,

    /// Whether to dump selected cells of interest
    #[structopt(short = "c", long)]
    pub(crate) dumpcells: bool,

    /// Verbosity level
    #[structopt(short, long, default_value = "Info", help = "Verbosity level.\nPossible values with increasing amount of output are\n'off', 'error', 'warn', 'info', 'debug', 'trace'.")]
    pub(crate) loglevel: log::LevelFilter,

    /// Input files
    #[structopt(name = "INFILES", parse(from_os_str))]
    pub(crate) infiles: Vec<PathBuf>,
}

impl Opt {
    pub fn jet_def(&self) -> JetDefinition {
        JetDefinition {
            algo: self.jetalgorithm,
            r: self.jetradius,
            minpt: self.jetpt,
        }
    }
}
