mod opt;

use std::cell::RefCell;
use std::rc::Rc;

use crate::opt::{Opt, Search};

use anyhow::{Context, Result};
use clap::Parser;
use cres::{
    cell_collector::CellCollector,
    distance::{EuclWithScaledPt, PtDistance},
    hepmc2,
    prelude::*,
    neighbour_search::{NeighbourData, NeighbourSearch, NaiveNeighbourSearch, TreeSearch},
    resampler::DefaultResamplerBuilder, GIT_BRANCH, GIT_REV, VERSION,
};
use env_logger::Env;
use log::{debug, info};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use noisy_float::prelude::*;

fn main() -> Result<()> {
    let opt = Opt::parse();
    match opt.search {
        Search::Naive => run_main::<NaiveNeighbourSearch>(opt),
        Search::Tree => run_main::<TreeSearch>(opt),
    }?;
    info!("done");
    Ok(())
}

fn run_main<N>(opt: Opt) -> Result<()>
where
    N: NeighbourData,
    for <'x, 'y, 'z> &'x mut N: NeighbourSearch<PtDistance<'y, 'z, EuclWithScaledPt>>,
    for <'x, 'y, 'z> <&'x mut N as NeighbourSearch<PtDistance<'y, 'z, EuclWithScaledPt>>>::Iter: Iterator<Item=(usize, N64)>,
{
    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);

    rayon::ThreadPoolBuilder::new()
        .num_threads(opt.threads)
        .build_global()?;

    if let (Some(rev), Some(branch)) = (GIT_REV, GIT_BRANCH) {
        info!("cres {} rev {} ({})", VERSION, rev, branch);
    } else {
        info!("cres {}", VERSION);
    }

    debug!("settings: {:#?}", opt);

    let cell_collector = if opt.dumpcells {
        Some(Rc::new(RefCell::new(CellCollector::new())))
    } else {
        None
    };
    let resampler = DefaultResamplerBuilder::default()
        .max_cell_size(opt.max_cell_size)
        .num_partitions(opt.partitions)
        .ptweight(opt.ptweight)
        .strategy(opt.strategy)
        .weight_norm(opt.weight_norm)
        .cell_collector(cell_collector.clone())
        .neighbour_search::<N>()
        .build();

    let rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);

    let writer = hepmc2::WriterBuilder::default()
        .to_filename(&opt.outfile)
        .with_context(|| {
            format!("Failed to open {:?} for writing", opt.outfile)
        })?
        .weight_norm(opt.weight_norm)
        .cell_collector(cell_collector)
        .compression(opt.compression)
        .build()?;

    let reader = hepmc2::Reader::from_filenames(opt.infiles.iter().rev())?;
    let converter = hepmc2::ClusteringConverter::new(opt.jet_def.into());
    let unweighter = Unweighter::new(opt.unweight.minweight, rng);
    let mut cres = CresBuilder {
        reader,
        converter,
        resampler,
        unweighter,
        writer,
    }.build();
    cres.run()?;
    Ok(())
}
