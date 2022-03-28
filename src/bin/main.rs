mod opt;

use std::cell::RefCell;
use std::rc::Rc;

use crate::opt::Opt;

use anyhow::{Context, Result};
use clap::Parser;
use cres::{
    cell_collector::CellCollector, hepmc2, prelude::*,
    resampler::DefaultResamplerBuilder, GIT_BRANCH, GIT_REV, VERSION,
};
use env_logger::Env;
use log::{debug, info};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;

fn main() -> Result<()> {
    let opt = Opt::parse();
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

    debug!("settings: {:?}", opt);

    let mut resampler = DefaultResamplerBuilder::default();
    resampler
        .max_cell_size(opt.max_cell_size)
        .ptweight(opt.ptweight)
        .strategy(opt.strategy)
        .weight_norm(opt.weight_norm);
    if opt.dumpcells {
        resampler
            .cell_collector(Some(Rc::new(RefCell::new(CellCollector::new()))));
    }
    let resampler = resampler.build()?;

    let rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);

    let writer = hepmc2::WriterBuilder::default()
        .to_filename(&opt.outfile)
        .with_context(|| {
            format!("Failed to open {:?} for writing", opt.outfile)
        })?
        .weight_norm(opt.weight_norm)
        .cell_collector(resampler.cell_collector())
        .compression(opt.compression)
        .build()?;

    let mut cres = CresBuilder {
        reader: hepmc2::Reader::from_filenames(opt.infiles.iter().rev())?,
        converter: hepmc2::ClusteringConverter::new(opt.jet_def.into()),
        resampler,
        unweighter: Unweighter::new(opt.unweight.minweight, rng),
        writer,
    }
    .build();
    cres.run()?;
    info!("done");
    Ok(())
}
