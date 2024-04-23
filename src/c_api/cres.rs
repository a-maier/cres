// clippy warns against casts e.g. from `c_double` to `f64` in 1.67.1
#![allow(clippy::unnecessary_cast)]
use crate::c_api::distance::DistanceFn;
use crate::c_api::error::LAST_ERROR;
use crate::cluster::{self, DefaultClustering};
use crate::distance::{Distance, EuclWithScaledPt};
use crate::io::{Converter, IOBuilder};
use crate::prelude::{CresBuilder, NO_UNWEIGHTING};
use crate::resampler::{NoObserver, ResamplerBuilder};
use crate::seeds::StrategicSelector;
use crate::traits::Resample;

use crate::neighbour_search::{NaiveNeighbourSearch, TreeSearch};

use std::convert::From;
use std::ffi::{CStr, OsStr};
use std::fs::create_dir_all;
use std::os::raw::{c_char, c_double};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use anyhow::{anyhow, Error};
use log::debug;
use noisy_float::prelude::*;

/// Resampling options
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Opt {
    /// Names of input files
    infiles: *mut *mut c_char,
    /// Number of input files
    n_infiles: usize,
    /// Name of output directory
    outdir: *mut c_char,
    /// Which distance function to use
    ///
    /// If set to `NULL`, the default distance function from
    /// [arXiv:2109.07851](https://arxiv.org/abs/2109.07851)
    /// is used
    distance: *mut DistanceFn,
    /// Extra contribution to distance proportional to difference in pt
    ///
    /// This parameter is ignored when using a custom distance. Otherwise,
    /// it corresponds to the τ parameter of
    /// [arXiv:2109.07851](https://arxiv.org/abs/2109.07851)
    ptweight: c_double,
    /// Jet definition
    jet_def: JetDefinition,
    /// Algorithm for finding nearest-neigbour events,
    neighbour_search: Search,
    /// Maximum cell radius
    ///
    /// Set to INFINITY for unlimited cell sizes
    max_cell_size: c_double,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct JetDefinition {
    /// Jet algorithm
    pub algorithm: JetAlgorithm,
    /// Jet radius parameter
    pub radius: c_double,
    /// Minimum jet transverse momentum
    pub min_pt: c_double,
}

/// Nearest-neighbour search algorithms
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Search {
    /// Vantage point tree search
    Tree,
    /// Naive search
    Naive,
}

impl From<JetDefinition> for cluster::JetDefinition {
    fn from(j: JetDefinition) -> Self {
        Self {
            algorithm: j.algorithm.into(),
            radius: j.radius as f64,
            min_pt: j.min_pt as f64,
        }
    }
}

/// Jet clustering algorithms
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum JetAlgorithm {
    /// The [anti-kt](https://arxiv.org/abs/0802.1189) algorithm
    AntiKt,
    /// The [Cambridge](https://arxiv.org/abs/hep-ph/9707323)/[Aachen](https://arxiv.org/abs/hep-ph/9907280) algorithm
    CambridgeAachen,
    /// The [kt](https://arxiv.org/abs/hep-ph/9305266) algorithm
    Kt,
}

impl From<JetAlgorithm> for cluster::JetAlgorithm {
    fn from(j: JetAlgorithm) -> Self {
        use crate::cluster::JetAlgorithm::*;
        match j {
            JetAlgorithm::AntiKt => AntiKt,
            JetAlgorithm::CambridgeAachen => CambridgeAachen,
            JetAlgorithm::Kt => Kt,
        }
    }
}

/// Run the cell resampler with the given options
///
/// # Return values
///
/// - `0`: success
/// - Non-zero: an error occurred, check with `cres_get_last_err` or
///   `cres_print_last_err`
#[no_mangle]
#[must_use]
pub extern "C" fn cres_run(opt: &Opt) -> i32 {
    match std::panic::catch_unwind(|| cres_run_internal(opt)) {
        Ok(Ok(())) => 0,
        Ok(Err(err)) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = Some(err));
            1
        }
        Err(err) => {
            LAST_ERROR
                .with(|e| *e.borrow_mut() = Some(anyhow!("panic: {:?}", err)));
            -1
        }
    }
}

fn cres_run_internal(opt: &Opt) -> Result<(), Error> {
    if opt.distance.is_null() {
        debug!("Using built-in distance function");
        cres_run_with_dist(opt, EuclWithScaledPt::new(n64(opt.ptweight)))
    } else {
        let distance = unsafe { *opt.distance };
        debug!("Using custom distance function {distance:?}");
        cres_run_with_dist(opt, distance)
    }
}

fn cres_run_with_dist<D>(opt: &Opt, dist: D) -> Result<(), Error>
where
    D: Distance + Send + Sync,
{
    match opt.neighbour_search {
        Search::Tree => cres_run_with::<D, TreeSearch>(opt, dist),
        Search::Naive => cres_run_with::<D, NaiveNeighbourSearch>(opt, dist),
    }
}

type Resampler<D, N> =
    crate::resampler::Resampler<D, N, NoObserver, StrategicSelector>;

fn cres_run_with<D, N>(opt: &Opt, dist: D) -> Result<(), Error>
where
    D: Distance + Send + Sync,
    Resampler<D, N>: Resample,
    <Resampler<D, N> as Resample>::Error:
        std::error::Error + Send + Sync + 'static,
{
    debug!("Settings: {:#?}", opt);

    let infiles: Vec<_> = unsafe {
        let names = std::slice::from_raw_parts(opt.infiles, opt.n_infiles);
        names
            .iter()
            .map(|&p| OsStr::from_bytes(CStr::from_ptr(p).to_bytes()))
            .collect()
    };
    debug!("Will read input from {:?}", infiles);

    let outdir = unsafe { CStr::from_ptr(opt.outdir) };
    let outdir = OsStr::from_bytes(outdir.to_bytes());
    debug!("Will write output to {:?}", outdir);
    create_dir_all(outdir)?;

    let files = infiles.into_iter().map(|f| {
        let out =
            PathBuf::from_iter([outdir, PathBuf::from(f).file_name().unwrap()]);
        (f, out)
    });

    // TODO: multiple weights, output compression
    let event_io = IOBuilder::default().build_from_files_iter(files)?;

    let converter = Converter::new();
    let clustering = DefaultClustering::new(opt.jet_def.into());

    // TODO: unweighting
    let unweighter = NO_UNWEIGHTING;

    // TODO: seeds, observer
    let resampler = ResamplerBuilder::default()
        .max_cell_size(Some(opt.max_cell_size as f64))
        .distance(dist)
        .neighbour_search::<N>()
        .build();

    let mut cres = CresBuilder {
        event_io,
        converter,
        clustering,
        resampler,
        unweighter,
    }
    .build();
    debug!("Starting resampler");
    cres.run()?;

    Ok(())
}
