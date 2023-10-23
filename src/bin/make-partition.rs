mod opt_common;

use std::{path::PathBuf, fs::File};

use crate::opt_common::*;

use anyhow::{Result, Context, bail};
use clap::Parser;
use cres::{FEATURES, GIT_REV, GIT_BRANCH, VERSION, reader::FileReader, event::Event, distance::{EuclWithScaledPt, DistWrapper}, vptree::VPTree, partition::{VPTreePartition, VPBisection}, compression::{Compression, compress_writer}, prelude::DefaultClustering, storage::Converter, traits::{TryConvert, Clustering}};
use env_logger::Env;
use log::{info, debug, trace};
use noisy_float::prelude::*;
use rayon::prelude::IntoParallelIterator;

// TODO: code duplication with opt::Opt
#[derive(Debug, Parser)]
#[clap(about, author, version)]
struct Opt {
    /// Output file.
    #[clap(long, short, value_parser)]
    outfile: PathBuf,

    #[clap(flatten)]
    jet_def: JetDefinition,

    #[clap(flatten)]
    lepton_def: LeptonDefinition,

    #[clap(flatten)]
    photon_def: PhotonDefinition,

    /// Include neutrinos in the distance measure
    #[clap(long, default_value_t)]
    include_neutrinos: bool,

    /// Number of partitions
    ///
    /// The input event sample is split into the given number of
    /// partitions, which has to be a power of two. Each partition is
    /// written to its own output file.
    #[clap(long, value_parser = parse_npartitions)]
    partitions: u32,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    infiles: Vec<PathBuf>,

    #[clap(long, value_parser = parse_compr,
           help = "Compress output file.
Possible settings are 'bzip2', 'gzip', 'zstd', 'lz4'.
Compression levels can be set with algorithm_level e.g. 'zstd_5'.
Maximum levels are 'gzip_9', 'zstd_19', 'lz4_16'.")]
    compression: Option<Compression>,

    /// Verbosity level
    #[clap(
        short,
        long,
        default_value = "Info",
        help = "Verbosity level.
Possible values with increasing amount of output are
'off', 'error', 'warn', 'info', 'debug', 'trace'.\n"
    )]
    loglevel: String,

    #[clap(
        short,
        long,
        default_value_t,
        help = "Number of threads.

If set to 0, a default number of threads is chosen.
The default can be set with the `RAYON_NUM_THREADS` environment
variable."
    )]
    threads: usize,

    /// Weight of transverse momentum when calculating particle momentum distances.
    #[clap(long, default_value = "0.")]
    ptweight: f64,
}

fn main() -> Result<()> {
    let args = argfile::expand_args_from(
        std::env::args_os(),
        argfile::parse_fromfile,
        argfile::PREFIX,
    )
    .with_context(|| "Failed to read argument file")?;
    let opt = Opt::parse_from(args);
    // TODO: validate!

    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);

    rayon::ThreadPoolBuilder::new()
        .num_threads(opt.threads)
        .build_global()?;

    if let (Some(rev), Some(branch)) = (GIT_REV, GIT_BRANCH) {
        info!("cres-make-partition {VERSION} rev {rev} ({branch}) {FEATURES:?}");
    } else {
        info!("cres-make-partition {VERSION} {FEATURES:?}");
    }

    debug!("settings: {:#?}", opt);

    let converter = Converter::new();

    let mut clustering = DefaultClustering::new(opt.jet_def.into())
        .include_neutrinos(opt.include_neutrinos);
    if opt.lepton_def.leptonalgorithm.is_some() {
        clustering = clustering.with_lepton_def(opt.lepton_def.into())
    }
    if opt.photon_def.photonradius.is_some() {
        clustering = clustering.with_photon_def(opt.photon_def.into())
    }

    // TODO: in principle we only need the kinematic part
    let mut events = Vec::new();
    for file in opt.infiles {
        let reader = FileReader::new(&file)?;
        for event in reader {
            let event = event.with_context(
                || format!("Failed to read event from {file:?}")
            )?;
            let weight = event.weights.first().and_then(|w| w.weight);
            match weight {
                Some(w) if w < 0.0 => {
                    let event: Event = converter.try_convert(event)?;
                    let event = clustering.cluster(event)?;
                    events.push(event)
                },
                _ => {}
            }
        }
    }

    if (opt.partitions as usize) > events.len() {
        bail!(
            "Number of negative-weight events ({}) must be at least as large as number of partitions ({})",
            events.len(),
            opt.partitions,
        )
    }
    let nevents = events.len();

    info!("Constructing {} partitions from {nevents} negative-weight events", opt.partitions);
    let depth = log2(opt.partitions);
    let distance = EuclWithScaledPt::new(n64(opt.ptweight));
    let dist = DistWrapper::new(&distance, &events);
    let partition = VPTree::from_par_iter_with_dist_and_depth(
        (0..events.len()).into_par_iter(),
        dist,
        depth as usize
    );
    info!("Converting to output format");
    let partition = VPTreePartition::from(partition);

    let tree = partition.into_tree();
    let tree = Vec::from_iter(
        tree.into_iter()
            .map(|VPBisection{pt, r}| {
                let pt = std::mem::take(&mut events[pt]);
                VPBisection{pt, r}
            })
    );

    // Safety: we have changed neither the structure of the tree nor
    // the distance function
    let partition = unsafe {
        VPTreePartition::from_vp(tree, distance)
    };
    trace!("Partition: {partition:#?}");

    let outfile = opt.outfile;
    info!("Writing to {outfile:?}");
    let out = File::create(&outfile).with_context(
        || format!("Failed to open {outfile:?}")
    )?;
    let out = compress_writer(out, opt.compression).with_context(
        || format!("Failed to compress output to {outfile:?}")
    )?;
    serde_yaml::to_writer(out, &(clustering, partition))?;
    info!("Done");
    Ok(())
}

fn parse_npartitions(s: &str) -> Result<u32, String> {
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

/// Logarithm in base 2, rounded down
pub const fn log2(n: u32) -> u32 {
    u32::BITS - n.leading_zeros() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn tst_log2of0() {
        log2(0);
    }

    #[test]
    fn tst_log2() {
        assert_eq!(log2(1), 0);
        for n in 2..=3 {
            assert_eq!(log2(n), 1);
        }
        for n in 4..=7 {
            assert_eq!(log2(n), 2);
        }
    }
}
