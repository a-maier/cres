mod opt_common;
mod opt_particle_def;
mod opt_split;

use std::{
    collections::{hash_map::Entry, HashMap},
    fs,
    path::{Path, PathBuf},
};

use crate::opt_split::Opt;

use anyhow::{Context, Result};
use clap::Parser;
use cres::{
    event::Event, io::Converter, ntuple::NTupleConverter,
    prelude::DefaultClustering, traits::Clustering, ParticleID, FEATURES,
    GIT_BRANCH, GIT_REV, VERSION,
};
use env_logger::Env;
use itertools::Itertools;
use log::{debug, info};
use opt_particle_def::ParticleDefinitions;

fn main() -> Result<()> {
    // TODO: code duplication with other cres binaries
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
        info!(
            "cres-split-by-multiplicity {VERSION} rev {rev} ({branch}) {FEATURES:?}"
        );
    } else {
        info!("cres-split-by-multiplicity {VERSION} {FEATURES:?}");
    }

    debug!("settings: {:#?}", opt);

    let converter = Converter::new();

    let ParticleDefinitions {
        jet_def,
        lepton_def,
        photon_def,
        include_neutrinos,
        min_missing_pt,
        reconstruct_W,
    } = opt.particle_def;
    let mut clustering = DefaultClustering::new(jet_def.into())
        .reconstruct_W(reconstruct_W)
        .include_neutrinos(include_neutrinos)
        .min_missing_pt(min_missing_pt);
    if lepton_def.leptonalgorithm.is_some() {
        clustering = clustering.with_lepton_def(lepton_def.into())
    }
    if photon_def.photonradius.is_some() {
        clustering = clustering.with_photon_def(photon_def.into())
    }

    for file in opt.infiles {
        info!("Splitting up {file:?}");

        let reader = ntuple::Reader::new(&file).with_context(|| {
            format!("Failed to read {file:?} as NTuple file")
        })?;
        let filename = Path::new(file.file_name().unwrap());
        let mut writers = HashMap::new();

        for event in reader {
            let event = event.with_context(|| {
                format!("Failed to read event from {file:?}")
            })?;
            let internal = converter.convert_ntuple(event.clone())?;
            let clustered = clustering.cluster(internal)?;
            let multiplicities = multiplicities(&clustered);
            match writers.entry(multiplicities) {
                Entry::Vacant(v) => {
                    let mult = v.key();
                    let out_path = gen_out_path(&opt.outdir, mult, filename);
                    fs::create_dir_all(out_path.parent().unwrap())?;
                    let writer = ntuple::Writer::new(&out_path, "")
                        .with_context(|| {
                            format!(
                                "Failed to write {out_path:?} as NTuple file"
                            )
                        })?;
                    v.insert(writer).write(&event)?
                }
                Entry::Occupied(mut o) => o.get_mut().write(&event)?,
            };
        }
    }
    info!("Done");
    Ok(())
}

fn gen_out_path(
    outdir: &Path,
    mult: &[(ParticleID, usize)],
    filename: &Path,
) -> PathBuf {
    let mut mult_string = mult
        .iter()
        .map(|(id, n)| format!("{n}_{}", name(*id).replace(' ', "_")))
        .join("_");
    if mult_string.is_empty() {
        mult_string = "no_particles".to_owned();
    }
    [&outdir, Path::new(&mult_string), Path::new(filename)]
        .into_iter()
        .collect()
}

fn multiplicities(event: &Event) -> Vec<(ParticleID, usize)> {
    event
        .outgoing()
        .iter()
        .map(|(id, p)| (*id, p.len()))
        .collect()
}

// TODO: code duplication with `cres.rs`
fn name(t: ParticleID) -> String {
    use cres::cluster;
    t.name()
        .map(|n| format!("{n}s"))
        .unwrap_or_else(|| match t {
            cluster::PID_JET => "jets".to_string(),
            cluster::PID_DRESSED_LEPTON => "dressed leptons".to_string(),
            cluster::PID_ISOLATED_PHOTON => "isolated photons".to_string(),
            _ => format!("particles with id {}", t.id()),
        })
}
