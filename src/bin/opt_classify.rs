use std::path::PathBuf;

use clap::Parser;
use cres::compression::Compression;

use crate::opt_common::parse_compr;

// TODO: code duplication with opt::Opt
#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub(crate) struct Opt {
    /// File containing partitioning information
    ///
    /// This is a file created with `cres-make-partition`
    #[clap(long, short, value_parser)]
    pub(crate) partition: PathBuf,

    /// Output directory.
    ///
    /// For each input file `prefix.suffix`, output is written to
    /// files `prefix.X.suffix`, where X is a number identifying the
    /// region. `prefix` is the filename component before the first
    /// `.`
    #[clap(long, short, value_parser)]
    pub(crate) outdir: PathBuf,

    #[clap(short = 'c', long, value_parser = parse_compr,
                help = "Compress output files.
Possible settings are 'bzip2', 'gzip', 'zstd', 'lz4'
Compression levels can be set with algorithm_level e.g. 'zstd_5'.
Maximum levels are 'gzip_9', 'zstd_19', 'lz4_16'.")]
    pub(crate) compression: Option<Compression>,

    /// Verbosity level
    ///
    /// Possible values with increasing amount of output are
    /// 'off', 'error', 'warn', 'info', 'debug', 'trace'.
    #[clap(short, long, default_value = "Info")]
    pub(crate) loglevel: String,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    pub(crate) infiles: Vec<PathBuf>,
}
