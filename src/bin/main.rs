mod auto_decompress;
mod hepmc;
mod opt;
mod progress_bar;
mod unweight;

use crate::hepmc::{into_event, CombinedReader};
use crate::opt::Opt;
use crate::progress_bar::{Progress, ProgressBar};
use crate::unweight::unweight;

use std::collections::{hash_map::Entry, HashMap};
use std::fs::File;
use std::io::BufWriter;

use env_logger::Env;
use hepmc2::writer::Writer;
use log::{debug, info, trace};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rayon::prelude::*;
use structopt::StructOpt;

use cres::cell::Cell;
use cres::distance::distance;
// use cres::parser::parse_event;

fn median_radius(cells: &[Cell]) -> N64 {
    let mut radii: Vec<_> = cells.iter().map(|c| c.radius()).collect();
    radii.sort_unstable();
    radii[radii.len() / 2]
}

const NUM_DUMP_CELLS: usize = 10;

fn select_dump_cells(cells: &mut [Cell]) -> HashMap<usize, Vec<usize>> {
    let mut res: HashMap<_, Vec<_>> = HashMap::new();
    info!("Cells by creation order:");
    for cell in cells.iter().take(NUM_DUMP_CELLS) {
        info!(
            "Cell {} with {} events and radius {} and weight {:e}",
            cell.id(),
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        for event in cell.iter() {
            res.entry(event.id).or_default().push(cell.id())
        }
    }

    info!("Largest cells by radius:");
    cells.select_nth_unstable_by_key(NUM_DUMP_CELLS - 1, |c| -c.radius());
    cells[..NUM_DUMP_CELLS].sort_unstable_by_key(|c| -c.radius());
    for cell in cells.iter().take(NUM_DUMP_CELLS) {
        info!(
            "Cell {} with {} events and radius {} and weight {:e}",
            cell.id(),
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        for event in cell.iter() {
            res.entry(event.id).or_default().push(cell.id())
        }
    }

    info!("Largest cells by number of events:");
    let cmp = |c1: &Cell, c2: &Cell| c2.nmembers().cmp(&c1.nmembers());
    cells.select_nth_unstable_by(NUM_DUMP_CELLS - 1, cmp);
    cells[..NUM_DUMP_CELLS].sort_unstable_by(cmp);
    for cell in cells.iter().take(NUM_DUMP_CELLS) {
        info!(
            "Cell {} with {} events and radius {} and weight {:e}",
            cell.id(),
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        for event in cell.iter() {
            res.entry(event.id).or_default().push(cell.id())
        }
    }

    info!("Cells with largest accumulated weights:");
    cells.select_nth_unstable_by_key(NUM_DUMP_CELLS - 1, |c| -c.weight_sum());
    cells[..NUM_DUMP_CELLS].sort_unstable_by_key(|c| -c.weight_sum());
    for cell in cells.iter().take(NUM_DUMP_CELLS) {
        info!(
            "Cell {} with {} events and radius {} and weight {:e}",
            cell.id(),
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        for event in cell.iter() {
            res.entry(event.id).or_default().push(cell.id())
        }
    }

    //TODO: info!("Randomly selected cells:");
    res
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
    let infiles: Result<Vec<_>, _> =
        opt.infiles.iter().rev().map(File::open).collect();
    let infiles = infiles?;
    let mut reader = CombinedReader::new(infiles);
    for (id, event) in (&mut reader).enumerate() {
        trace!("read event {}", id);
        let mut event = into_event(event?, &opt.jet_def);
        event.id = id;
        events.push(event);
    }

    info!("Read {} events", events.len());

    let orig_sum_wt: N64 = events.iter().map(|e| e.weight).sum();
    let orig_sum_wt2: N64 = events.iter().map(|e| e.weight * e.weight).sum();

    info!(
        "Initial sum of weights: {:e} ± {:e}",
        orig_sum_wt,
        orig_sum_wt2.sqrt()
    );

    let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
    let progress = ProgressBar::new(nneg_weight as u64, "events treated:");

    let mut cells = Vec::new();
    while let Some((n, _)) =
        events.par_iter().enumerate().min_by_key(|(_n, e)| e.weight)
    {
        if events[n].weight > 0. {
            break;
        }
        let seed = events.swap_remove(n);

        let mut event_dists: Vec<_> = events
            .into_par_iter()
            .map(|e| (distance(&e, &seed), e))
            .collect();

        let mut cell = Cell::from_seed(cells.len(), seed);
        debug!("Cell seed with weight {:e}", cell.weight_sum());

        while cell.weight_sum() < 0. {
            let nearest = event_dists
                .par_iter()
                .enumerate()
                .min_by_key(|(_n, (d, _e))| d);
            let nearest_idx =
                if let Some((n, _)) = nearest { n } else { break };
            let (dist, event) = event_dists.swap_remove(nearest_idx);
            trace!(
                "adding event with distance {}, weight {:e} to cell",
                dist,
                event.weight
            );
            cell.push_with_dist(event, dist)
        }
        progress.inc(cell.iter().filter(|e| e.weight < 0.).count() as u64);
        debug!(
            "New cell with {} events, radius {}, and weight {:e}",
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        cells.push(cell);

        events = event_dists.into_par_iter().map(|(_, e)| e).collect();
        debug!("{} events left", events.len());
    }
    progress.finish();
    info!("Created {} cells", cells.len());
    info!("Median radius: {}", median_radius(&cells));

    info!("Resampling");
    cells.par_iter_mut().for_each(|cell| cell.resample());

    let dump_event_to = if opt.dumpcells {
        select_dump_cells(&mut cells)
    } else {
        HashMap::new()
    };

    info!("Collecting and sorting events");
    for cell in cells {
        events.append(&mut cell.into());
    }
    events.par_sort_unstable();

    if opt.unweight.minweight > 0.0 {
        info!("Unweight to minimum weight {:e}", opt.unweight.minweight);
        let mut rng = Xoshiro256Plus::seed_from_u64(opt.unweight.seed);
        unweight(&mut events, opt.unweight.minweight, &mut rng);
    }

    let final_sum_wt: N64 = events.iter().map(|e| e.weight).sum();
    let final_sum_wt2: N64 = events.iter().map(|e| e.weight * e.weight).sum();

    info!(
        "Final sum of weights: {:e} ± {:e}",
        final_sum_wt,
        final_sum_wt2.sqrt()
    );

    info!("Writing {} events to {:?}", events.len(), opt.outfile);
    reader.rewind()?;
    let outfile = BufWriter::new(File::create(opt.outfile)?);
    let mut cell_writers = HashMap::new();
    for cellnr in dump_event_to.values().flatten() {
        if let Entry::Vacant(entry) = cell_writers.entry(cellnr) {
            let file = File::create(format!("cell{}.hepmc", cellnr))?;
            let writer = Writer::try_from(BufWriter::new(file))?;
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
                let (_, ev) = hepmc_events.next().unwrap();
                ev.unwrap();
            }
            let (id, ev) = hepmc_events.next().unwrap();
            debug_assert_eq!(id, event.id);
            hepmc_event = ev.unwrap();
        }
        let old_weight = hepmc_event.weights.first().unwrap();
        let reweight: f64 = (event.weight / old_weight).into();
        for weight in &mut hepmc_event.weights {
            *weight *= reweight
        }
        writer.write(&hepmc_event)?;
        let cellnums: &[usize] = dump_event_to
            .get(&event.id)
            .map(|v| v.as_slice())
            .unwrap_or_default();
        for cellnum in cellnums {
            let cell_writer = cell_writers.get_mut(cellnum).unwrap();
            cell_writer.write(&hepmc_event)?;
        }
        progress.inc(1);
    }
    progress.finish();
    info!("done");
    Ok(())
}
