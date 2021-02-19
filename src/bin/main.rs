mod progress_bar;
mod hepmc;
mod opt;

use crate::progress_bar::{Progress, ProgressBar};
use crate::hepmc::{into_event, CombinedReader};
use crate::opt::Opt;

use std::fs::File;
use std::io::BufWriter;

use log::{debug, info, trace};
use hepmc2::writer::Writer;
use noisy_float::prelude::*;
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

fn main() {
    env_logger::init();

    let opt = Opt::from_args();
    log::set_max_level(opt.loglevel);
    debug!("settings: {:?}", opt);

    let mut events = Vec::new();

    debug!("Reading events from {:?}", opt.infiles);
    let infiles = opt.infiles.iter().rev().map(
        |f| File::open(f).unwrap()
    ).collect();
    let mut reader = CombinedReader::new(infiles);
    for (id, event) in (&mut reader).enumerate() {
        trace!("read event {}", id);
        let mut event = into_event(event.unwrap(), &opt.jet_def);
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

        let mut cell = Cell::from_seed(seed);
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

    info!("Collecting and sorting events");
    for cell in cells {
        events.append(&mut cell.into());
    }
    events.par_sort_unstable();

    let final_sum_wt: N64 = events.iter().map(|e| e.weight).sum();
    let final_sum_wt2: N64 = events.iter().map(|e| e.weight * e.weight).sum();

    info!(
        "Final sum of weights: {:e} ± {:e}",
        final_sum_wt,
        final_sum_wt2.sqrt()
    );

    info!("Writing {} events to {:?}", events.len(), opt.outfile);
    reader.rewind().unwrap();
    let outfile = BufWriter::new(File::create(opt.outfile).unwrap());
    let mut writer = Writer::try_from(outfile).unwrap();
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
        let reweight: f64 = (event.weight/old_weight).into();
        for weight in &mut hepmc_event.weights {
            *weight *= reweight
        }
        writer.write(&hepmc_event).unwrap();
        progress.inc(1);
    }
    progress.finish();
    info!("done");
}
