use std::path::PathBuf;

use clap::Parser;
use cres::compression::Compression;

use crate::{opt_common::*, opt_particle_def::ParticleDefinitions};

// TODO: code duplication with opt_cres
#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub(crate) struct Opt {
    /// Output file.
    #[clap(long, short, value_parser)]
    pub(crate) outfile: PathBuf,

    /// Number of regions into which the phase space is partitioned
    #[clap(long, value_parser = parse_nregions)]
    pub(crate) regions: u32,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    pub(crate) infiles: Vec<PathBuf>,

    #[clap(flatten)]
    pub(crate) particle_def: ParticleDefinitions,

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

    /// Weight of transverse momentum when calculating particle momentum distances.
    #[clap(long, default_value = "0.")]
    pub(crate) ptweight: f64,
}

fn parse_nregions(s: &str) -> Result<u32, String> {
    use std::str::FromStr;

    match u32::from_str(s) {
        Ok(n) => {
            if n.is_power_of_two() {
                Ok(n)
            } else {
                Err("has to be a power of two".to_string())
            }
        }
        Err(err) => Err(err.to_string()),
    }
}
