mod progress_bar;
mod hepmc;

use crate::progress_bar::get_progress_bar;
use crate::hepmc::from;

use log::{debug, info, trace};
use std::fs::File;
use std::io::{BufReader, Write};

use bzip2::read::BzDecoder;
use hepmc2::reader::Reader;
use noisy_float::prelude::*;
use rayon::prelude::*;

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

    let args: Vec<String> = std::env::args().collect();

    let mut events = Vec::new();

    for arg in &args[1..args.len() - 1] {
        info!("Reading events from {}", arg);
        let decoder = BzDecoder::new(BufReader::new(File::open(arg).unwrap()));
        let reader = Reader::from(BufReader::new(decoder));
        for (id, event) in reader.enumerate() {
            let mut event = from(event.unwrap());
            event.id = id;
            events.push(event)
        }
    }

    let mut writer = File::create(args.last().unwrap()).unwrap();
    info!("Read {} events", events.len());

    let orig_sum_wt: N64 = events.iter().map(|e| e.weight).sum();
    let orig_sum_wt2: N64 = events.iter().map(|e| e.weight * e.weight).sum();

    info!(
        "Initial sum of weights: {:e} ± {:e}",
        orig_sum_wt,
        orig_sum_wt2.sqrt()
    );

    let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
    let mut progress = get_progress_bar(nneg_weight as u64, "events treated:");

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
        progress.as_mut()
            .map(|p| p.inc(cell.iter().filter(|e| e.weight < 0.).count() as u64));
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
    progress.map(|p| p.finish());
    info!("Created {} cells", cells.len());
    info!("Median radius: {}", median_radius(&cells));

    info!("Resampling");
    cells.par_iter_mut().for_each(|cell| cell.resample());

    info!("Collecting and sorting events");
    for cell in cells {
        events.append(&mut cell.into());
    }
    events.par_sort_unstable();

    info!("Writing output...");
    for event in events {
        writeln!(writer, "{} {:e}", event.id, event.weight).unwrap();
    }
}
