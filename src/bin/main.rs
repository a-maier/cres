mod auto_decompress;
mod hepmc;
mod opt;
mod progress_bar;
mod unweight;
mod cell_collector;
mod writer;

use crate::hepmc::{into_event, CombinedReader};
use crate::opt::Opt;
use crate::progress_bar::{Progress, ProgressBar};
use crate::unweight::unweight;
use crate::cell_collector::CellCollector;
use crate::writer::make_writer;

use std::collections::{hash_map::Entry, HashMap};
use std::fs::File;
use std::io::BufWriter;

use env_logger::Env;
use hepmc2::writer::Writer;
use log::{debug, info, trace, warn};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rayon::prelude::*;
use structopt::StructOpt;

use cres::cell::Cell;
use cres::distance::EuclWithScaledPt;
// use cres::parser::parse_event;

fn median_radius(radii: &mut [N64]) -> N64 {
    radii.sort_unstable();
    radii[radii.len() / 2]
}

fn main() {
    if let Err(err) = run_main() {
        eprintln!("ERROR: {}", err)
    }
}

fn run_main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();
    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);

    debug!("settings: {:?}", opt);

    let mut events = Vec::new();

    debug!("Reading events from {:?}", opt.infiles);
    if opt.infiles.len() > 1 {
        warn!(
            "Dividing all weights by number of input files ({})",
            opt.infiles.len()
        );
    }
    let infiles: Result<Vec<_>, _> =
        opt.infiles.iter().rev().map(File::open).collect();
    let infiles = infiles?;
    let mut reader = CombinedReader::new(infiles);
    for (id, event) in (&mut reader).enumerate() {
        trace!("read event {}", id);
        let mut event = into_event(event?, &opt.jet_def);
        event.id = id;
        event.weight /= opt.infiles.len() as f64;
        events.push(event);
    }

    info!("Read {} events", events.len());

    let xs: N64 = events.iter().map(|e| e.weight).sum();
    let xs = n64(opt.weight_norm) * xs;
    let sum_wtsqr: N64 = events.iter().map(|e| e.weight * e.weight).sum();
    let xs_err = n64(opt.weight_norm) * sum_wtsqr.sqrt();
    info!("Initial cross section: σ = {:.3e} ± {:.3e}", xs, xs_err);

    let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
    let progress = ProgressBar::new(nneg_weight as u64, "events treated:");

    let mut cell_radii = Vec::new();
    let mut cell_collector = CellCollector::new();
    let mut rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);
    let mut events: Vec<_> = events.into_par_iter().map(|e| (n64(0.), e)).collect();
    let distance = EuclWithScaledPt::new(n64(opt.ptweight));
    while let Some((mut cell, _)) = Cell::new(&mut events, &distance, opt.strategy) {
        progress.inc(cell.nneg_weights() as u64);
        debug!(
            "New cell with {} events, radius {}, and weight {:e}",
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        cell_radii.push(cell.radius());
        cell.resample();
        if opt.dumpcells { cell_collector.collect(&cell, &mut rng); }
    }
    progress.finish();
    info!("Created {} cells", cell_radii.len());
    info!("Median radius: {:.3}", median_radius(cell_radii.as_mut_slice()));

    if opt.dumpcells { cell_collector.dump_info(); }
    let dump_event_to = cell_collector.event_cells();

    info!("Collecting and sorting events");
    let mut events: Vec<_> = events.into_par_iter().map(|(_dist, event)| event).collect();
    events.par_sort_unstable();

    if opt.unweight.minweight > 0.0 {
        info!("Unweight to minimum weight {:e}", opt.unweight.minweight);
        unweight(&mut events, opt.unweight.minweight, &mut rng);
    }

    let sum_wt: N64 = events.par_iter().map(|e| e.weight).sum();
    let xs = n64(opt.weight_norm) * sum_wt;
    let sum_wtsqr: N64 = events.par_iter().map(|e| e.weight * e.weight).sum();
    let xs_err = n64(opt.weight_norm) * sum_wtsqr.sqrt();
    info!("Final cross section: σ = {:.3e} ± {:.3e}", xs, xs_err);

    info!("Writing {} events to {:?}", events.len(), opt.outfile);
    reader.rewind()?;
    let outfile = File::create(opt.outfile)?;
    let outfile = make_writer(BufWriter::new(outfile), opt.compression)?;
    let mut cell_writers = HashMap::new();
    for cellnr in dump_event_to.values().flatten() {
        if let Entry::Vacant(entry) = cell_writers.entry(cellnr) {
            let file = File::create(format!("cell{}.hepmc", cellnr))?;
            let writer = make_writer(BufWriter::new(file), opt.compression)?;
            let writer = Writer::try_from(writer)?;
            entry.insert(writer);
        }
    }
    let mut writer = Writer::try_from(outfile)?;
    let mut hepmc_events = reader.enumerate();
    let progress = ProgressBar::new(events.len() as u64, "events written:");
    for event in events {
        let (hepmc_id, hepmc_event) = hepmc_events.next().unwrap();
        let mut hepmc_event = hepmc_event.unwrap();
        if hepmc_id < event.id {
            for _ in hepmc_id..event.id {
                let (_id, ev) = hepmc_events.next().unwrap();
                hepmc_event = ev.unwrap();
            }
        }
        let old_weight = hepmc_event.weights.first().unwrap();
        let reweight: f64 = (event.weight / old_weight).into();
        for weight in &mut hepmc_event.weights {
            *weight *= reweight
        }
        hepmc_event.xs.cross_section = xs.into();
        hepmc_event.xs.cross_section_error = xs_err.into();
        writer.write(&hepmc_event)?;
        let cellnums: &[usize] = dump_event_to
            .get(&event.id)
            .map(|v: &Vec<usize>| v.as_slice())
            .unwrap_or_default();
        for cellnum in cellnums {
            let cell_writer = cell_writers.get_mut(cellnum).unwrap();
            cell_writer.write(&hepmc_event)?;
        }
        progress.inc(1);
    }
    writer.finish()?;
    for (_, cell_writer) in cell_writers {
        cell_writer.finish()?;
    }
    progress.finish();
    info!("done");
    Ok(())
}
