use std::cell::RefCell;
use std::default::Default;
use std::fmt::{self, Display};
use std::rc::Rc;

use crate::cell::Cell;
use crate::cell_collector::CellCollector;
use crate::distance::EuclWithScaledPt;
use crate::traits::Resample;
use crate::event::Event;
use crate::progress_bar::{Progress, ProgressBar};

use derive_builder::Builder;
use log::{debug, info, warn};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rayon::prelude::*;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Strategy {
    LeastNegative,
    MostNegative,
    Next,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::MostNegative
    }
}

#[derive(Debug, Clone)]
pub struct UnknownStrategy(pub String);

impl Display for UnknownStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown strategy: {}", self.0)
    }
}

#[derive(Builder)]
pub struct DefaultResampler {
    #[builder(default = "1.")]
    weight_norm: f64,
    #[builder(default = "0.")]
    ptweight: f64,
    #[builder(default)]
    strategy: Strategy,
    #[builder(default)]
    max_cell_size: Option<f64>,
    #[builder(default)]
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
}

fn median_radius(radii: &mut [N64]) -> N64 {
    radii.sort_unstable();
    radii[radii.len() / 2]
}

impl Resample for DefaultResampler {
    type Error = std::convert::Infallible;

    fn resample(&mut self, events: Vec<Event>) -> Result<Vec<Event>, Self::Error> {
        let xs: N64 = events.iter().map(|e| e.weight).sum();
        let xs = n64(self.weight_norm) * xs;
        let sum_wtsqr: N64 = events.iter().map(|e| e.weight * e.weight).sum();
        let xs_err = n64(self.weight_norm) * sum_wtsqr.sqrt();
        info!("Initial cross section: σ = {:.3e} ± {:.3e}", xs, xs_err);

        let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
        let progress = ProgressBar::new(nneg_weight as u64, "events treated:");

        let mut cell_radii = Vec::new();
        let seeds = choose_seeds(&events, self.strategy);
        let mut events: Vec<_> = events.into_par_iter().map(|e| (n64(0.), e)).collect();
        let distance = EuclWithScaledPt::new(n64(self.ptweight));
        let max_cell_size = n64(self.max_cell_size.unwrap_or(f64::MAX));
        let mut rng = Xoshiro256Plus::seed_from_u64(0);
        let mut nneg = 0;
        for seed in seeds {
            progress.inc(1);
            if events[seed].1.weight > 0. { continue }
            let mut cell = Cell::new(&mut events, seed, &distance, max_cell_size);
            debug!(
                "New cell with {} events, radius {}, and weight {:e}",
                cell.nmembers(),
                cell.radius(),
                cell.weight_sum()
            );
            cell_radii.push(cell.radius());
            if cell.weight_sum() < 0. { nneg += 1 }
            cell.resample();
            self.cell_collector.as_ref().map(|c| c.borrow_mut().collect(&cell, &mut rng));
        }
        progress.finish();
        info!("Created {} cells", cell_radii.len());
        if nneg > 0 { warn!("{} cells had negative weight!", nneg); }
        info!("Median radius: {:.3}", median_radius(cell_radii.as_mut_slice()));
        self.cell_collector.as_ref().map(|c| c.borrow().dump_info());

        info!("Collecting and sorting events");
        let mut events: Vec<_> = events.into_par_iter().map(|(_dist, event)| event).collect();
        events.par_sort_unstable();
        Ok(events)
    }
}

impl DefaultResampler {
    pub fn cell_collector(&self) -> Option<Rc<RefCell<CellCollector>>> {
        self.cell_collector.as_ref().map(|c| c.clone())
    }
}

fn choose_seeds(events: &[Event], strategy: Strategy) -> Vec<usize> {
    use Strategy::*;
    let mut neg_weight: Vec<_> = events.par_iter().enumerate().filter(
        |(_n, e)| e.weight < 0.
    ).map(|(n, _e)| n).collect();
    match strategy {
        Next => {},
        MostNegative => neg_weight.par_sort_unstable_by_key(|&n| events[n].weight),
        LeastNegative => neg_weight
            .par_sort_unstable_by(|&n, &m| events[m].weight.cmp(&events[n].weight)),
    }
    neg_weight
}
