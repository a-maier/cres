mod opt_common;
mod opt_cres;
mod opt_cres_validate;
mod opt_particle_def;

use std::error::Error;
use std::fs::create_dir_all;
use std::path::PathBuf;

use crate::opt_cres::{Opt, Search};
use crate::opt_cres_validate::validate;

use anyhow::{Context, Result};
use clap::Parser;
use cres::cluster::DefaultClustering;
use cres::io::{Converter, IOBuilder};
use cres::resampler::DefaultResampler;
use cres::traits::Resample;
use cres::{
    neighbour_search::{NaiveNeighbourSearch, TreeSearch},
    prelude::*,
    resampler::DefaultResamplerBuilder,
    FEATURES, GIT_BRANCH, GIT_REV, VERSION,
};
use env_logger::Env;
use log::{debug, info};
use opt_particle_def::ParticleDefinitions;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;

fn main() -> Result<()> {
    let args = argfile::expand_args_from(
        std::env::args_os(),
        argfile::parse_fromfile,
        argfile::PREFIX,
    )
    .with_context(|| "Failed to read argument file")?;
    let opt = validate(Opt::parse_from(args))?;
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
    DefaultResampler<N>: Resample,
    <DefaultResampler<N> as Resample>::Error: Error + Send + Sync + 'static,
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
    let ParticleDefinitions {
        jet_def,
        lepton_def,
        photon_def,
        include_neutrinos,
    } = opt.particle_def;

    let mut event_io = IOBuilder::default();
    #[cfg(feature = "multiweight")]
    event_io.weight_names(opt.weights.clone());
    event_io.compression(opt.compression);

    create_dir_all(&opt.outdir)?;
    let files = opt.infiles.into_iter().map(|f| {
        let out = PathBuf::from_iter([
            opt.outdir.as_os_str(),
            f.file_name().unwrap(),
        ]);
        (f, out)
    });
    let event_io = event_io.build_from_files_iter(files)?;

    let cell_collector = None;
    // let cell_collector = if opt.dumpcells {
    //     Some(Rc::new(RefCell::new(CellCollector::new())))
    // } else {
    //     None
    // };
    let resampler = DefaultResamplerBuilder::default()
        .max_cell_size(opt.max_cell_size)
        .ptweight(opt.ptweight)
        .strategy(opt.strategy)
        .cell_collector(cell_collector.clone())
        .neighbour_search::<N>()
        .build();

    let rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);

    let unweighter = Unweighter::new(opt.unweight.minweight, rng);
    let mut clustering = DefaultClustering::new(jet_def.into())
        .include_neutrinos(include_neutrinos);
    if lepton_def.leptonalgorithm.is_some() {
        clustering = clustering.with_lepton_def(lepton_def.into())
    }
    if photon_def.photonradius.is_some() {
        clustering = clustering.with_photon_def(photon_def.into())
    }
    #[cfg(feature = "multiweight")]
    let converter = Converter::with_weights(opt.weights);
    #[cfg(not(feature = "multiweight"))]
    let converter = Converter::new();

    let mut cres = CresBuilder {
        event_io,
        converter,
        clustering,
        resampler,
        unweighter,
    }
    .build();
    cres.run()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cres() {
        use std::path::PathBuf;

        use cres::cluster::JetAlgorithm;
        use tempfile::tempdir;

        use crate::opt_common::{
            JetDefinition, LeptonDefinition, PhotonDefinition,
        };

        let tempdir = tempdir().unwrap();
        let opt = Opt {
            outdir: tempdir.path().to_path_buf(),
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
            photon_def: PhotonDefinition {
                photonefrac: Some(0.09),
                photonradius: Some(0.2),
                photonpt: Some(20.),
            },
            max_cell_size: Some(100.),
            infiles: vec![PathBuf::from("test_data/showered.hepmc.zst")],
            include_neutrinos: Default::default(),
            unweight: Default::default(),
            ptweight: Default::default(),
            // dumpcells: Default::default(),
            compression: Default::default(),
            loglevel: "info".to_owned(),
            search: Default::default(),
            strategy: Default::default(),
            threads: Default::default(),
            #[cfg(feature = "multiweight")]
            weights: Default::default(),
        };
        cres(opt).unwrap();
    }
}
