use crate::prelude::{CresBuilder, NO_UNWEIGHTING};
use crate::hepmc2;
use crate::distance::EuclWithScaledPt;
use crate::resampler::ResamplerBuilder;
use crate::c_api::error::LAST_ERROR;
use crate::c_api::distance::DistanceFn;

use std::convert::From;
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::os::raw::{c_char, c_double};

use anyhow::{anyhow, Error};
use log::debug;
use noisy_float::prelude::*;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Opt {
    infiles: *mut *mut c_char,
    n_infiles: usize,
    outfile: *mut c_char,
    distance: *mut DistanceFn,
    ptweight: c_double,
    jet_def: JetDefinition,
    weight_norm: c_double,
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

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum JetAlgorithm {
    AntiKt,
    CambridgeAachen,
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

#[no_mangle]
#[must_use]
pub extern "C" fn cres_run(opt: &Opt) -> i32 {
    match std::panic::catch_unwind(
        || cres_run_internal(opt)
    ) {
        Ok(Ok(())) => 0,
        Ok(Err(err)) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = Some(err));
            1
        },
        Err(err) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = Some(anyhow!("panic: {:?}", err)));
            -1
        }
    }
}

fn cres_run_internal(opt: &Opt) -> Result<(), Error> {
    debug!("Settings: {:#?}", opt);

    let infiles: Vec<_> = unsafe {
        let names = std::slice::from_raw_parts(opt.infiles, opt.n_infiles);
        names.into_iter().map(|&p| CStr::from_ptr(p)).collect()
    };
    debug!("Will read input from {:?}", infiles);

    let outfile = unsafe {
        CStr::from_ptr(opt.outfile)
    };
    let outfile = OsStr::from_bytes(outfile.to_bytes());
    debug!("Will write output to {:?}", outfile);
    let outfile = std::fs::File::create(outfile)?;

    let reader = hepmc2::Reader::from_filenames(infiles.iter().rev().map(
        |f| OsStr::from_bytes(f.to_bytes())
    ))?;

    let converter = hepmc2::ClusteringConverter::new(opt.jet_def.into());

    // TODO: unweighting
    let unweighter = NO_UNWEIGHTING;

    let writer = hepmc2::WriterBuilder::default()
        .writer(outfile)
        .weight_norm(opt.weight_norm)
        .build()?;

    // TODO: seeds, observer
    let resampler = ResamplerBuilder::default()
        .weight_norm(opt.weight_norm)
        .max_cell_size(Some(opt.max_cell_size as f64));

    // TODO: code duplication
    if !opt.distance.is_null() {
        let distance = unsafe { *opt.distance };
        debug!("Using custom distance function {:?}", distance);
        let resampler = resampler
            .distance(distance)
            .build();
        let mut cres = CresBuilder {
            reader,
            converter,
            resampler,
            unweighter,
            writer
        }.build();
        debug!("Starting resampler");
        cres.run()?;
    } else {
        debug!("Using built-in distance function");
        let resampler = resampler.distance(
            EuclWithScaledPt::new(n64(opt.ptweight))
        ).build();
        let mut cres = CresBuilder {
            reader,
            converter,
            resampler,
            unweighter,
            writer
        }.build();
        debug!("Starting resampler");
        cres.run()?;
    }

    Ok(())
}
