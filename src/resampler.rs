use std::cell::RefCell;
use std::default::Default;
use std::rc::Rc;

use crate::cell::Cell;
use crate::cell_collector::CellCollector;
use crate::distance::{Distance, EuclWithScaledPt};
use crate::traits::Resample;
use crate::event::Event;
use crate::progress_bar::{Progress, ProgressBar};
use crate::seeds::{Strategy, StrategicSelector};
use crate::traits::{ObserveCell, SelectSeeds};

use derive_builder::Builder;
use log::{debug, info, warn};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rayon::prelude::*;

/// Main resampling class
pub struct Resampler<D, O, S>  {
    seeds: S,
    distance: D,
    observer: O,
    weight_norm: f64,
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

impl<D, O, S, T> Resample for Resampler<D, O, S>
where
    D: Distance + Send + Sync,
    S: SelectSeeds<Iter=T>,
    T: Iterator<Item=usize>,
    O: ObserveCell,
{
    type Error = std::convert::Infallible;

    /// Resampling
    ///
    /// For each seed, we construct a cell as described in
    /// [arXiv:2109.07851](https://arxiv.org/abs/2109.07851).
    /// Seeds with non-negative weight are ignored.
    fn resample(&mut self, events: Vec<Event>) -> Result<Vec<Event>, Self::Error> {
        self.print_xs(&events);

        let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
        let progress = ProgressBar::new(nneg_weight as u64, "events treated:");

        let max_cell_size = n64(self.max_cell_size.unwrap_or(f64::MAX));

        let seeds = self.seeds.select_seeds(&events);
        let mut events: Vec<_> = events.into_par_iter().map(|e| (n64(0.), e)).collect();
        for seed in seeds.take(nneg_weight) {
            if seed >= events.len() { break }
            progress.inc(1);
            if events[seed].1.weight > 0. { continue }
            let mut cell = Cell::new(
                &mut events,
                seed,
                &self.distance,
                max_cell_size
            );
            cell.resample();
            self.observer.observe_cell(&cell);
        }
        progress.finish();
        self.observer.finish();

        let events: Vec<_> = events.into_par_iter().map(
            |(_dist, event)| event
        ).collect();
        Ok(events)
    }
}

/// Construct a `Resampler` object
pub struct ResamplerBuilder<D, O, S> {
    seeds: S,
    distance: D,
    observer: O,
    weight_norm: f64,
    max_cell_size: Option<f64>,
}

impl<D, O, S> ResamplerBuilder<D, O, S> {
    /// Build the `Resampler`
    pub fn build(self) -> Resampler<D, O, S> {
        Resampler {
            seeds: self.seeds,
            distance: self.distance,
            observer: self.observer,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define how and in which order to choose cell seeds
    pub fn seeds<SS, T>(self, seeds: SS) -> ResamplerBuilder<D, O, SS>
    where
        SS: SelectSeeds<Iter=T>,
        T: Iterator<Item=usize>,
    {
        ResamplerBuilder {
            seeds,
            distance: self.distance,
            observer: self.observer,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define the distance between events
    pub fn distance<DD>(self, distance: DD) -> ResamplerBuilder<DD, O, S>
    where DD: Distance + Send + Sync
    {
        ResamplerBuilder {
            seeds: self.seeds,
            distance,
            observer: self.observer,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Callback that will be applied to each constructed cell after resampling
    pub fn observer<OO>(self, observer: OO) -> ResamplerBuilder<D, OO, S>
    where OO: ObserveCell
    {
        ResamplerBuilder {
            seeds: self.seeds,
            distance: self.distance,
            observer,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define the ratio between the cross section and the sum of weights
    ///
    /// The default is 1.
    pub fn weight_norm(self, weight_norm: f64) -> ResamplerBuilder<D, O, S> {
        ResamplerBuilder {
            weight_norm,
            ..self
        }
    }

    /// Set a maximum cell radius
    ///
    /// The default is `None`, meaning unlimited cell size.
    pub fn max_cell_size(self, max_cell_size: Option<f64>) -> ResamplerBuilder<D, O, S> {
        ResamplerBuilder {
            max_cell_size,
            ..self
        }
    }
}

impl Default for ResamplerBuilder<EuclWithScaledPt, NoObserver, StrategicSelector> {
    fn default() -> Self {
        Self {
            seeds: Default::default(),
            distance: Default::default(),
            observer: Default::default(),
            weight_norm: 1.,
            max_cell_size: Default::default(),
        }
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

        let mut resampler = ResamplerBuilder::default()
            .seeds(StrategicSelector::new(self.strategy))
            .distance(EuclWithScaledPt::new(n64(self.ptweight)))
            .observer(observer)
            .weight_norm(self.weight_norm)
            .max_cell_size(self.max_cell_size)
            .build();
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

impl ObserveCell for Observer {
    fn observe_cell(&mut self, cell: &Cell) {
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

#[derive(Copy, Clone, Default, Debug)]
pub struct NoObserver { }
impl ObserveCell for NoObserver {
    fn observe_cell(&mut self, _cell: &Cell) { }
}

/// Default cell observer doing nothing
pub const NO_OBSERVER: NoObserver = NoObserver { };
