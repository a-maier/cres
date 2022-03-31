use crate::c_api::distance::DistanceFn;
use crate::c_api::error::LAST_ERROR;
use crate::distance::EuclWithScaledPt;
use crate::hepmc2;
use crate::neighbour_search::NaiveNeighbourSearch;
use crate::prelude::{CresBuilder, NO_UNWEIGHTING};
use crate::resampler::ResamplerBuilder;

use std::convert::From;
use std::ffi::{CStr, OsStr};
use std::os::raw::{c_char, c_double};
use std::os::unix::ffi::OsStrExt;

use anyhow::{anyhow, Error};
use log::debug;
use noisy_float::prelude::*;

/// Resampling options
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Opt {
    /// Names of input files
    ///
    /// Input files should be in HepMC2 format,
    /// possibly compressed with bzip2, gzip, lz4, or zstd
    infiles: *mut *mut c_char,
    /// Number of input files
    n_infiles: usize,
    /// Name of HepMC output file
    outfile: *mut c_char,
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
    /// How to get from weights to the cross section: σ = `weight_norm` * (sum of weights)
    weight_norm: c_double,
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

impl From<JetDefinition> for hepmc2::converter::JetDefinition {
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

impl From<JetAlgorithm> for hepmc2::converter::JetAlgorithm {
    fn from(j: JetAlgorithm) -> Self {
        use crate::hepmc2::converter::JetAlgorithm::*;
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
    debug!("Settings: {:#?}", opt);

    let infiles: Vec<_> = unsafe {
        let names = std::slice::from_raw_parts(opt.infiles, opt.n_infiles);
        names.iter().map(|&p| CStr::from_ptr(p)).collect()
    };
    debug!("Will read input from {:?}", infiles);

    let outfile = unsafe { CStr::from_ptr(opt.outfile) };
    let outfile = OsStr::from_bytes(outfile.to_bytes());
    debug!("Will write output to {:?}", outfile);
    let outfile = std::fs::File::create(outfile)?;

    let reader = hepmc2::Reader::from_filenames(
        infiles
            .iter()
            .rev()
            .map(|f| OsStr::from_bytes(f.to_bytes())),
    )?;

    let converter = hepmc2::ClusteringConverter::new(opt.jet_def.into());

    // TODO: unweighting
    let unweighter = NO_UNWEIGHTING;

    let writer = hepmc2::WriterBuilder::default()
        .writer(outfile)
        .weight_norm(opt.weight_norm)
        .build()?;

    // TODO: seeds, observer
    let resampler = ResamplerBuilder::<_,_,_,NaiveNeighbourSearch>::default()
        .weight_norm(opt.weight_norm)
        .max_cell_size(Some(opt.max_cell_size as f64));

    // TODO: code duplication
    if !opt.distance.is_null() {
        let distance = unsafe { *opt.distance };
        debug!("Using custom distance function {:?}", distance);
        let resampler = resampler.distance(distance).build();
        let mut cres = CresBuilder {
            reader,
            converter,
            resampler,
            unweighter,
            writer,
        }
        .build();
        debug!("Starting resampler");
        cres.run()?;
    } else {
        debug!("Using built-in distance function");
        let resampler = resampler
            .distance(EuclWithScaledPt::new(n64(opt.ptweight)))
            .build();
        let mut cres = CresBuilder {
            reader,
            converter,
            resampler,
            unweighter,
            writer,
        }
        .build();
        debug!("Starting resampler");
        cres.run()?;
    }

    Ok(())
}
