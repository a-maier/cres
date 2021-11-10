mod opt;

use std::rc::Rc;
use std::cell::RefCell;

use crate::opt::Opt;

use anyhow::{Context, Result};
use cres::{
    prelude::*,
    resampler::DefaultResamplerBuilder,
    cell_collector::CellCollector,
    VERSION, GIT_REV, GIT_BRANCH
};
use log::{info, debug};
use env_logger::Env;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use structopt::StructOpt;
use noisy_float::prelude::*;

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);
    info!("cres {} rev {} ({})", VERSION, GIT_REV, GIT_BRANCH);

    debug!("settings: {:?}", opt);

    let mut resampler = DefaultResamplerBuilder::default();
    resampler.max_cell_size(opt.max_cell_size)
        .ptweight(opt.ptweight)
        .strategy(opt.strategy)
        .weight_norm(opt.weight_norm);
    if opt.dumpcells {
        resampler.cell_collector(Some(Rc::new(RefCell::new(CellCollector::new()))));
    }
    let resampler = resampler.build()?;

    let rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);

    let writer = HepMCWriterBuilder::default()
        .to_filename(&opt.outfile).with_context(
            || format!("Failed to open {:?} for writing", opt.outfile)
        )?
        .weight_norm(opt.weight_norm)
        .cell_collector(resampler.cell_collector())
        .compression(opt.compression)
        .build()?;

    let mut cres = CresBuilder {
        reader: HepMCReader::from_filenames(opt.infiles.iter().rev())?,
        converter: HepMCConverter::new(opt.jet_def.into(), n64(opt.ptweight)),
        resampler,
        unweighter: Unweighter::new(opt.unweight.minweight, rng),
        writer
    }.build();
    cres.run()?;
    info!("done");
    Ok(())
}
