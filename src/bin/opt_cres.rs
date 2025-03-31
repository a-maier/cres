use std::fmt::{self, Display};
use std::path::PathBuf;

use crate::opt_common::*;
use crate::opt_particle_def::ParticleDefinitions;

use cres::compression::Compression;
use cres::seeds::Strategy;

use clap::{Parser, ValueEnum};
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
pub struct UnknownStrategy(pub String);

impl Display for UnknownStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown strategy: {}", self.0)
    }
}

#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, ValueEnum,
)]
pub(crate) enum Search {
    #[default]
    Tree,
    Naive,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, ValueEnum)]
pub(crate) enum DistanceType {
    #[default]
    Absolute,
    Relative,
}

#[derive(Debug, Default, Copy, Clone, Parser)]
pub(crate) struct UnweightOpt {
    /// Weight below which events are unweighted. '0' means no unweighting.
    #[clap(short = 'w', long, default_value = "0.")]
    pub(crate) minweight: f64,

    /// Random number generator seed for unweighting.
    #[clap(long, default_value = "0")]
    pub(crate) seed: u64,
}

#[derive(Debug, Parser)]
#[clap(about, author, version)]
#[allow(non_snake_case)]
pub(crate) struct Opt {
    /// Output directory.
    ///
    /// For each input file, an output file with the same name is
    /// written to the given directory.
    #[clap(long, short, value_parser)]
    pub(crate) outdir: PathBuf,

    #[clap(flatten)]
    pub(crate) particle_def: ParticleDefinitions,

    #[clap(flatten)]
    pub(crate) unweight: UnweightOpt,

    /// Weight of transverse momentum when calculating particle momentum distances.
    #[clap(long, default_value = "0.")]
    pub(crate) ptweight: f64,

    /// Whether to dump selected cells of interest.
    #[clap(short = 'd', long)]
    pub(crate) dumpcells: bool,

    #[clap(long, value_parser = parse_compr,
                help = "Compress output file.
Possible settings are 'bzip2', 'gzip', 'zstd', 'lz4'.
Compression levels can be set with algorithm_level e.g. 'zstd_5'.
Maximum levels are 'gzip_9', 'zstd_19', 'lz4_16'.")]
    pub(crate) compression: Option<Compression>,

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

    /// Algorithm for finding nearest-neighbour events.
    #[clap(value_enum, short, long, default_value = "tree")]
    pub(crate) search: Search,

    #[clap(
        long, default_value = "most_negative",
        value_parser = parse_strategy,
        help = "Strategy for choosing cell seeds. Possible values are
'least_negative': event with negative weight closest to zero,
'most_negative' event with the lowest weight,
'any': no additional requirements beyond a negative weight.\n"
    )]
    pub(crate) strategy: Strategy,

    #[clap(
        short,
        long,
        default_value_t,
        help = "Number of threads.

If set to 0, a default number of threads is chosen.
The default can be set with the `RAYON_NUM_THREADS` environment
variable."
    )]
    pub(crate) threads: usize,

    /// Maximum cell size in GeV.
    ///
    /// Limiting the cell size ensures that event weights are only
    /// redistributed between events that are sufficiently similar.
    /// The downside is that not all negative weights may be cancelled.
    #[clap(long)]
    pub(crate) max_cell_size: Option<f64>,

    /// Whether to use an absolute or relative distance
    #[clap(long)]
    pub(crate) distance: DistanceType,

    /// Comma-separated list of weights to include in the resampling
    ///
    /// In addition to the main event weight, weights with the given
    /// names will be averaged within each cell.
    // Would be nice to use a HashSet here, but clap refuses to parse
    // that out of the box
    #[cfg(feature = "multiweight")]
    #[clap(long, value_delimiter = ',')]
    pub(crate) weights: Vec<String>,

    /// Discard events with zero weight
    ///
    /// By default, only even weights are modified and all other event
    /// information is, even for events with zero weight. This is
    /// important for cases where the event records carry additional
    /// information. For instance, HepMC events with zero weight may
    /// still update the current cross section estimate. With this
    /// option enabled, events with zero weight are omitted from the
    /// output.
    #[clap(long, default_value_t)]
    pub(crate) discard_weightless: bool,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    pub(crate) infiles: Vec<PathBuf>,
}
