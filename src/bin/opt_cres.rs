use std::fmt::{self, Display};
use std::path::PathBuf;

use crate::opt_common::*;
use crate::opt_particle_def::ParticleDefinitions;

use cres::compression::Compression;
use cres::seeds::{Strategy, WeightSign};

use clap::{Parser, ValueEnum};
use thiserror::Error;

fn parse_strategy(s: &str) -> Result<Strategy, UnknownStrategy> {
    use Strategy::*;
    match s {
        "Any" | "any" => Ok(Next),
        "LargestAbsWeightFirst"
        | "largest_abs_weight_first"
        | "MostNegative"
        | "most_negative" => Ok(LargestAbsWeightFirst),
        "SmallestAbsWeightFirst"
        | "smallest_abs_weight_first"
        | "LeastNegative"
        | "least_negative" => Ok(SmallestAbsWeightFirst),
        _ => Err(UnknownStrategy(s.to_string())),
    }
}

#[derive(Debug, Clone, Error)]
pub struct UnknownStrategy(pub String);

impl Display for UnknownStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown strategy: '{}'", self.0)
    }
}

fn parse_weight_sign(s: &str) -> Result<WeightSign, UnknownWeightSign> {
    use WeightSign::*;
    match s {
        "All" | "all" => Ok(All),
        "Negative" | "negative" => Ok(Negative),
        "Positive" | "positive" => Ok(Positive),
        _ => Err(UnknownWeightSign(s.to_string())),
    }
}

#[derive(Debug, Clone, Error)]
pub struct UnknownWeightSign(pub String);

impl Display for UnknownWeightSign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown weight sign: '{}'", self.0)
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

    // TODO: make this option work again
    // this will require some update to the `UpdateWeights` implementations
    // /// Whether to dump selected cells of interest.
    // #[clap(short = 'd', long)]
    // pub(crate) dumpcells: bool,
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
        long, default_value = "any",
        value_parser = parse_strategy,
        help = "Strategy for choosing cell seeds. Possible values are
'smallest_abs_weight_first': seeds with weight closest to zero are chosen first,
'largest_abs_weight_first' seeds with the largest absolute weight are chosen first,
'any': seeds are chosen in an arbitrary order.\n"
    )]
    pub(crate) strategy: Strategy,

    #[clap(
        long, default_value = "negative",
        value_parser = parse_weight_sign,
        help = "Which events are chosen as cell seeds. Possible values are
'negative': events with negative weight,
'positive': events with positive weight,
'all': all events, regardless of weight. The default is 'negative'."
    )]
    pub(crate) seed_weights: WeightSign,

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
