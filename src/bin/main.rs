mod opt;

use std::cell::RefCell;
#[cfg(feature = "multiweight")]
use std::collections::HashSet;
use std::rc::Rc;

use crate::opt::{Opt, Search};

use anyhow::{Context, Result};
use clap::Parser;
use cres::converter::ClusteringConverter;
use cres::reader::CombinedReader;
use cres::writer::FileWriter;
use cres::{
    cell_collector::CellCollector,
    distance::{EuclWithScaledPt, PtDistance},
    neighbour_search::{
        NaiveNeighbourSearch, NeighbourData, NeighbourSearch, TreeSearch,
    },
    prelude::*,
    resampler::DefaultResamplerBuilder,
    FEATURES, GIT_BRANCH, GIT_REV, VERSION,
};
use env_logger::Env;
use log::{debug, info};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;

fn main() -> Result<()> {
    let args = argfile::expand_args_from(
        std::env::args_os(),
        argfile::parse_fromfile,
        argfile::PREFIX,
    )
    .with_context(|| "Failed to read argument file")?;
    let opt = Opt::parse_from(args).validate()?;
    cres(opt)
}

fn cres(opt: Opt) -> Result<()> {
    match opt.search {
        Search::Naive => cres_with_search::<NaiveNeighbourSearch>(opt),
        Search::Tree => cres_with_search::<TreeSearch>(opt),
    }?;
    info!("done");
    Ok(())
}

fn cres_with_search<N>(opt: Opt) -> Result<()>
where
    N: NeighbourData + Clone + Send + Sync,
    for<'x, 'y, 'z> &'x N:
        NeighbourSearch<PtDistance<'y, 'z, EuclWithScaledPt>>,
    for<'x, 'y, 'z> <&'x N as NeighbourSearch<PtDistance<'y, 'z, EuclWithScaledPt>>>::Iter:
        Iterator<Item = (usize, N64)>,
{
    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);

    rayon::ThreadPoolBuilder::new()
        .num_threads(opt.threads)
        .build_global()?;

    if let (Some(rev), Some(branch)) = (GIT_REV, GIT_BRANCH) {
        info!("cres {VERSION} rev {rev} ({branch}) {FEATURES:?}");
    } else {
        info!("cres {VERSION} {FEATURES:?}");
    }

    debug!("settings: {:#?}", opt);

    let reader = CombinedReader::from_files(opt.infiles)?;

    let cell_collector = if opt.dumpcells {
        Some(Rc::new(RefCell::new(CellCollector::new())))
    } else {
        None
    };
    let resampler = DefaultResamplerBuilder::default()
        .max_cell_size(opt.max_cell_size)
        .ptweight(opt.ptweight)
        .strategy(opt.strategy)
        .cell_collector(cell_collector.clone())
        .neighbour_search::<N>()
        .build();

    let rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);

    let unweighter = Unweighter::new(opt.unweight.minweight, rng);
    #[cfg(feature = "multiweight")]
    let weights: HashSet<_> = opt.weights.into_iter().collect();
    let mut converter = ClusteringConverter::new(opt.jet_def.into())
        .include_neutrinos(opt.include_neutrinos);
    #[cfg(feature = "multiweight")]
    {
        converter = converter.include_weights(weights.clone());
    }
    if opt.lepton_def.leptonalgorithm.is_some() {
        converter = converter.with_lepton_def(opt.lepton_def.into())
    }
    let writer = FileWriter::builder()
        .filename(opt.outfile.clone())
        .format(opt.outformat.into())
        .compression(opt.compression)
        .cell_collector(cell_collector);
    #[cfg(feature = "multiweight")]
    let writer = writer.overwrite_weights(weights);
    let writer = writer.build();

    let mut cres = CresBuilder {
        reader,
        converter,
        resampler,
        unweighter,
        writer,
    }
    .build();
    cres.run()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_cres() {
        use std::path::PathBuf;

        use cres::cluster::JetAlgorithm;

        use crate::opt::{JetDefinition, LeptonDefinition};

        let opt = Opt {
            outfile: PathBuf::from("/dev/null"),
            jet_def: JetDefinition {
                jetalgorithm: JetAlgorithm::AntiKt,
                jetradius: 0.4,
                jetpt: 30.,
            },
            lepton_def: LeptonDefinition {
                leptonalgorithm: Some(JetAlgorithm::AntiKt),
                leptonradius: Some(0.1),
                leptonpt: Some(30.),
            },
            max_cell_size: Some(100.),
            infiles: vec![PathBuf::from("test_data/showered.hepmc.zst")],
            include_neutrinos: Default::default(),
            unweight: Default::default(),
            ptweight: Default::default(),
            dumpcells: Default::default(),
            compression: Default::default(),
            outformat: Default::default(),
            loglevel: "info".to_owned(),
            search: Default::default(),
            strategy: Default::default(),
            threads: Default::default(),
            weights: Default::default(),
        };
        cres(opt).unwrap();
    }
}
