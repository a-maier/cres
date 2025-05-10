use std::path::PathBuf;

use clap::Parser;

use crate::opt_particle_def::ParticleDefinitions;

// TODO: code duplication with opt_cres
#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub(crate) struct Opt {
    /// Output directory.
    #[clap(long, short, value_parser)]
    pub(crate) outdir: PathBuf,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    pub(crate) infiles: Vec<PathBuf>,

    #[clap(flatten)]
    pub(crate) particle_def: ParticleDefinitions,

    // TODO: output compression option

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
}
