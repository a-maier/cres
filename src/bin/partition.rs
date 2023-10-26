mod opt_common;
mod opt_partition;

use std::fs::File;

use crate::opt_partition::Opt;

use anyhow::{Result, Context, bail};
use clap::Parser;
use cres::{FEATURES, GIT_REV, GIT_BRANCH, VERSION, storage::FileReader, event::Event, distance::{EuclWithScaledPt, DistWrapper}, vptree::VPTree, partition::{VPTreePartition, VPBisection}, compression::compress_writer, prelude::DefaultClustering, storage::Converter, traits::{TryConvert, Clustering}};
use env_logger::Env;
use log::{info, debug, trace};
use noisy_float::prelude::*;
use rayon::prelude::IntoParallelIterator;

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
        let reader = FileReader::try_new(file.clone())?;
        for event in reader {
            let event = event.with_context(
                || format!("Failed to read event from {file:?}")
            )?;
            let event: Event = converter.try_convert(event)?;
            if event.weight() < 0.0 {
                let event = clustering.cluster(event)?;
                events.push(event)
            }
        }
    }

    if (opt.regions as usize) > events.len() {
        bail!(
            "Number of negative-weight events ({}) must be at least as large as number of regions ({})",
            events.len(),
            opt.regions,
        )
    }
    let nevents = events.len();

    info!("Constructing {} regions from {nevents} negative-weight events", opt.regions);
    let depth = log2(opt.regions);
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
