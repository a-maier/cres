use std::cell::RefCell;
use std::default::Default;
use std::fmt::{self, Display};
use std::rc::Rc;

use crate::cell::Cell;
use crate::cell_collector::CellCollector;
use crate::distance::{Distance, EuclWithScaledPt};
use crate::traits::Resample;
use crate::event::Event;
use crate::progress_bar::{Progress, ProgressBar};
use crate::traits::CellObserve;

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
pub struct Resampler<D, O, S>  {
    seeds: S,
    distance: D,
    observer: O,
    #[builder(default = "1.")]
    weight_norm: f64,
    #[builder(default)]
    max_cell_size: Option<f64>,
}

impl<D, O, S> Resampler<D, O, S> {
    fn print_xs(&self, events: &[Event]) {
        let xs: N64 = events.iter().map(|e| e.weight).sum();
        let xs = n64(self.weight_norm) * xs;
        let sum_wtsqr: N64 = events.iter().map(|e| e.weight * e.weight).sum();
        let xs_err = n64(self.weight_norm) * sum_wtsqr.sqrt();
        info!("Initial cross section: σ = {:.3e} ± {:.3e}", xs, xs_err);
    }
}

impl<D, O, S> Resample for Resampler<D, O, S>
where
    D: Distance + Send + Sync,
    S: Iterator<Item=usize>,
    O: CellObserve,
{
    type Error = std::convert::Infallible;

    fn resample(&mut self, events: Vec<Event>) -> Result<Vec<Event>, Self::Error> {
        self.print_xs(&events);

        let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
        let progress = ProgressBar::new(nneg_weight as u64, "events treated:");

        let max_cell_size = n64(self.max_cell_size.unwrap_or(f64::MAX));

        let mut events: Vec<_> = events.into_par_iter().map(|e| (n64(0.), e)).collect();
        for seed in &mut self.seeds {
            progress.inc(1);
            if events[seed].1.weight > 0. { continue }
            let mut cell = Cell::new(
                &mut events,
                seed,
                &self.distance,
                max_cell_size
            );
            cell.resample();
            self.observer.cell_observe(&cell);
        }
        progress.finish();
        self.observer.finish();

        info!("Collecting and sorting events");
        let mut events: Vec<_> = events.into_par_iter().map(
            |(_dist, event)| event
        ).collect();
        events.par_sort_unstable();
        Ok(events)
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

impl Resample for DefaultResampler {
    type Error = std::convert::Infallible;

    fn resample(&mut self, events: Vec<Event>) -> Result<Vec<Event>, Self::Error> {

        let mut observer = Observer::default();
        observer.cell_collector = self.cell_collector.clone();

        let seeds = choose_seeds(&events, self.strategy);
        let mut resampler = ResamplerBuilder::default()
            .seeds(seeds.iter().copied())
            .distance(EuclWithScaledPt::new(n64(self.ptweight)))
            .observer(observer)
            .weight_norm(self.weight_norm)
            .max_cell_size(self.max_cell_size)
            .build().unwrap();
        crate::traits::Resample::resample(&mut resampler, events)
    }
}

impl DefaultResampler {
    pub fn cell_collector(&self) -> Option<Rc<RefCell<CellCollector>>> {
        self.cell_collector.as_ref().map(|c| c.clone())
    }
}

fn median_radius(radii: &mut [N64]) -> N64 {
    radii.sort_unstable();
    radii[radii.len() / 2]
}

#[derive(Clone, Debug)]
struct Observer {
    cell_radii: Vec<N64>,
    rng: Xoshiro256Plus,
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    nneg: u64,
}

impl std::default::Default for Observer {
    fn default() -> Self {
        Self {
            cell_radii: Vec::new(),
            rng: Xoshiro256Plus::seed_from_u64(0),
            cell_collector: None,
            nneg: 0,
        }
    }
}

impl CellObserve for Observer {
    fn cell_observe(&mut self, cell: &Cell) {
        debug!(
            "New cell with {} events, radius {}, and weight {:e}",
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        self.cell_radii.push(cell.radius());
        if cell.weight_sum() < 0. { self.nneg += 1 }
        if let Some(c) = &self.cell_collector {
            c.borrow_mut().collect(&cell, &mut self.rng)
        }
    }

    fn finish(&mut self) {
        info!("Created {} cells", self.cell_radii.len());
        if self.nneg > 0 { warn!("{} cells had negative weight!", self.nneg); }
        info!("Median radius: {:.3}", median_radius(self.cell_radii.as_mut_slice()));
        self.cell_collector.as_ref().map(|c| c.borrow().dump_info());
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
