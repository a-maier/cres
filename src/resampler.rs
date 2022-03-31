use std::cell::RefCell;
use std::default::Default;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::bisect::circle_partition;
use crate::cell::Cell;
use crate::cell_collector::CellCollector;
use crate::distance::{Distance, EuclWithScaledPt, PtDistance};
use crate::event::Event;
use crate::neighbour_search::NaiveNeighbourSearch;
use crate::progress_bar::{Progress, ProgressBar};
use crate::seeds::{StrategicSelector, Strategy};
use crate::traits::{
    NeighbourData,
    NeighbourSearch,
    ObserveCell,
    Resample,
    SelectSeeds
};


use derive_builder::Builder;
use log::{debug, info, warn};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rayon::prelude::*;
use thiserror::Error;
use thread_local::ThreadLocal;

#[derive(Debug, Error)]
pub enum ResamplingError{
    #[error("Number of partitions is {0}, but has to be a power of two")]
    NPartition(u32)
}

/// Main resampling class
pub struct Resampler<D, N, O, S> {
    seeds: S,
    distance: D,
    neighbour_search: PhantomData<N>,
    observer: O,
    num_partitions: u32,
    weight_norm: f64,
    max_cell_size: Option<f64>,
}

impl<D, N, O, S> Resampler<D, N, O, S> {
    fn print_xs(&self, events: &[Event]) {
        let xs: N64 = events.iter().map(|e| e.weight).sum();
        let xs = n64(self.weight_norm) * xs;
        let sum_wtsqr: N64 = events.iter().map(|e| e.weight * e.weight).sum();
        let xs_err = n64(self.weight_norm) * sum_wtsqr.sqrt();
        info!("Initial cross section: σ = {:.3e} ± {:.3e}", xs, xs_err);
    }
}

impl<D, N, O, S, T> Resample for Resampler<D, N, O, S>
where
    D: Distance + Send + Sync,
    N: NeighbourData,
    for <'x, 'y, 'z> &'x mut N: NeighbourSearch<PtDistance<'y, 'z, D>>,
    for <'x, 'y, 'z> <&'x mut N as NeighbourSearch<PtDistance<'y, 'z, D>>>::Iter: Iterator<Item=(usize, N64)>,
    S: SelectSeeds<Iter = T> + Send + Sync,
    T: Iterator<Item = usize>,
    O: ObserveCell + Send + Sync,
{
    type Error = ResamplingError;

    /// Resampling
    ///
    /// For each seed, we construct a cell as described in
    /// [arXiv:2109.07851](https://arxiv.org/abs/2109.07851).
    /// Seeds with non-negative weight are ignored.
    fn resample(
        &mut self,
        mut events: Vec<Event>,
    ) -> Result<Vec<Event>, Self::Error> {
        if !self.num_partitions.is_power_of_two() {
            return Err(ResamplingError::NPartition(self.num_partitions))
        }
        self.print_xs(&events);

        let nneg_weight = events.iter().filter(|e| e.weight < 0.).count();
        let progress = ProgressBar::new(nneg_weight as u64, "events treated:");

        let max_cell_size = n64(self.max_cell_size.unwrap_or(f64::MAX));
        let depth = log2(self.num_partitions);
        let parts = circle_partition(
            &mut events,
            |e1, e2| (&self.distance).distance(e1, e2),
            depth
        );
        debug_assert_eq!(parts.len(), self.num_partitions as usize);

        parts.into_par_iter().for_each(|events| {
            let seeds = self.seeds.select_seeds(&events);
            let mut neighbour_search = N::new_with_dist(
                events.len(),
                PtDistance::new(&self.distance, &events)
            );
            for seed in seeds.take(nneg_weight) {
                if seed >= events.len() {
                    break;
                }
                progress.inc(1);
                if events[seed].weight > 0. {
                    continue;
                }

                let mut cell = Cell::new(
                    events,
                    seed,
                    &self.distance,
                    &mut neighbour_search,
                    max_cell_size
                );
                cell.resample();
                self.observer.observe_cell(&cell);
            }
        });
        progress.finish();
        self.observer.finish();

        Ok(events)
    }
}

/// Construct a `Resampler` object
pub struct ResamplerBuilder<D, O, S, N=NaiveNeighbourSearch> {
    seeds: S,
    distance: D,
    neighbour_search: PhantomData<N>,
    observer: O,
    weight_norm: f64,
    num_partitions: u32,
    max_cell_size: Option<f64>,
}

impl<D, O, S, N> ResamplerBuilder<D, O, S, N> {
    /// Build the `Resampler`
    pub fn build(self) -> Resampler<D, N, O, S> {
        Resampler {
            seeds: self.seeds,
            distance: self.distance,
            neighbour_search: PhantomData,
            observer: self.observer,
            num_partitions: self.num_partitions,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define how and in which order to choose cell seeds
    pub fn seeds<SS, T>(self, seeds: SS) -> ResamplerBuilder<D, O, SS, N>
    where
        SS: SelectSeeds<Iter = T>,
        T: Iterator<Item = usize>,
    {
        ResamplerBuilder {
            seeds,
            distance: self.distance,
            neighbour_search: PhantomData,
            observer: self.observer,
            num_partitions: self.num_partitions,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define the distance between events
    pub fn distance<DD>(self, distance: DD) -> ResamplerBuilder<DD, O, S, N>
    where
        DD: Distance + Send + Sync,
    {
        ResamplerBuilder {
            seeds: self.seeds,
            distance,
            neighbour_search: PhantomData,
            observer: self.observer,
            num_partitions: self.num_partitions,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Callback that will be applied to each constructed cell after resampling
    pub fn observer<OO>(self, observer: OO) -> ResamplerBuilder<D, OO, S, N>
    where
        OO: ObserveCell,
    {
        ResamplerBuilder {
            seeds: self.seeds,
            distance: self.distance,
            neighbour_search: PhantomData,
            observer,
            num_partitions: self.num_partitions,
            weight_norm: self.weight_norm,
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define the ratio between the cross section and the sum of weights
    ///
    /// The default is 1.
    pub fn weight_norm(self, weight_norm: f64) -> ResamplerBuilder<D, O, S, N> {
        ResamplerBuilder {
            weight_norm,
            ..self
        }
    }

    /// Define the number of partitions into which events should be split
    ///
    /// The default number of partitions is 1.
    pub fn num_partitions(self, num_partitions: u32) -> ResamplerBuilder<D, O, S, N> {
        ResamplerBuilder {
            num_partitions,
            ..self
        }
    }

    /// Set a maximum cell radius
    ///
    /// The default is `None`, meaning unlimited cell size.
    pub fn max_cell_size(
        self,
        max_cell_size: Option<f64>,
    ) -> ResamplerBuilder<D, O, S, N> {
        ResamplerBuilder {
            max_cell_size,
            ..self
        }
    }
}

impl<N> Default
    for ResamplerBuilder<EuclWithScaledPt, NoObserver, StrategicSelector, N>
{
    fn default() -> Self {
        Self {
            seeds: Default::default(),
            distance: Default::default(),
            neighbour_search: PhantomData,
            observer: Default::default(),
            weight_norm: 1.,
            num_partitions: 1,
            max_cell_size: Default::default(),
        }
    }
}

#[derive(Builder)]
pub struct DefaultResampler<N=NaiveNeighbourSearch> {
    #[builder(default = "1.")]
    weight_norm: f64,
    #[builder(default = "0.")]
    ptweight: f64,
    #[builder(default)]
    strategy: Strategy,
    #[builder(default)]
    max_cell_size: Option<f64>,
    #[builder(default = "1")]
    num_partitions: u32,
    #[builder(default)]
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    #[builder(default)]
    neighbour_search: PhantomData<N>,
}

impl<N> Resample for DefaultResampler<N>
where
    N: NeighbourData,
    for <'x, 'y, 'z> &'x mut N: NeighbourSearch<PtDistance<'y, 'z, EuclWithScaledPt>>,
    for <'x, 'y, 'z> <&'x mut N as NeighbourSearch<PtDistance<'y, 'z, EuclWithScaledPt>>>::Iter: Iterator<Item=(usize, N64)>,
{
    type Error = ResamplingError;

    fn resample(
        &mut self,
        events: Vec<Event>,
    ) -> Result<Vec<Event>, Self::Error> {

        let observer_data = ObserverData {
            cell_collector: self.cell_collector.clone().map(
                |c| c.borrow().clone()
            ),
            ..Default::default()
        };
        let observer = Observer {
            central: observer_data,
            threaded: Default::default()
        };

        let resampler: ResamplerBuilder<_,_,_,N> = ResamplerBuilder::default();
        let mut resampler = resampler
            .seeds(StrategicSelector::new(self.strategy))
            .distance(EuclWithScaledPt::new(n64(self.ptweight)))
            .num_partitions(self.num_partitions)
            .weight_norm(self.weight_norm)
            .max_cell_size(self.max_cell_size)
            .observer(observer)
            .build();
        let events = crate::traits::Resample::resample(
            &mut resampler,
            events
        )?;

        if let Some(c) = self.cell_collector.as_mut() {
            c.replace(
                resampler.observer.central.cell_collector.unwrap()
            );
        }
        Ok(events)
    }
}

impl<N> DefaultResampler<N> {
    pub fn cell_collector(&self) -> Option<Rc<RefCell<CellCollector>>> {
        self.cell_collector.as_ref().cloned()
    }
}

fn median_radius(radii: &mut [N64]) -> N64 {
    radii.sort_unstable();
    radii[radii.len() / 2]
}

#[derive(Debug, Default)]
struct Observer {
    central: ObserverData,
    threaded: ThreadLocal<RefCell<ObserverData>>
}

#[derive(Clone, Debug)]
struct ObserverData {
    cell_radii: Vec<N64>,
    rng: Xoshiro256Plus,
    cell_collector: Option<CellCollector>,
    nneg: u64,
}

impl std::default::Default for ObserverData {
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
    fn observe_cell(&self, cell: &Cell) {
        debug!(
            "New cell with {} events, radius {}, and weight {:e}",
            cell.nmembers(),
            cell.radius(),
            cell.weight_sum()
        );
        let mut data = self.threaded.get_or(
            || RefCell::new(self.central.clone())
        ).borrow_mut();
        data.cell_radii.push(cell.radius());
        if cell.weight_sum() < 0. {
            data.nneg += 1
        }
        let mut cell_collector = std::mem::take(&mut data.cell_collector);
        if let Some(c) = &mut cell_collector {
            c.collect(cell, &mut data.rng)
        }
        data.cell_collector = cell_collector;
    }

    fn finish(&mut self) {
        let data = std::mem::take(&mut self.threaded);
        let res = data.into_iter()
            .map(|c| c.into_inner())
            .reduce(|acc, c| acc.combine(c));
        if let Some(mut res) = res {
            info!("Created {} cells", res.cell_radii.len());
            if res.nneg > 0 {
                warn!("{} cells had negative weight!", res.nneg);
            }
            info!(
                "Median radius: {:.3}",
                median_radius(res.cell_radii.as_mut_slice())
            );
            res.cell_collector.as_ref().map(|c| c.dump_info());
            self.central = res;
        }
    }
}

impl ObserverData {
    pub fn combine(mut self, mut other: Self) -> Self {
        self.cell_radii.append(&mut other.cell_radii);
        self.nneg += other.nneg;
        self.cell_collector = match (self.cell_collector, other.cell_collector) {
            (Some(c1), Some(c2)) => Some(c1.combine(c2, &mut self.rng)),
            (Some(c), None) => Some(c),
            (None, Some(c)) => Some(c),
            (None, None) => None
        };
        self
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct NoObserver {}
impl ObserveCell for NoObserver {
    fn observe_cell(&self, _cell: &Cell) {}
}

/// Default cell observer doing nothing
pub const NO_OBSERVER: NoObserver = NoObserver {};

const fn log2(n: u32) -> u32 {
    u32::BITS - n.leading_zeros() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn tst_log2of0() {
        log2(0);
    }

    #[test]
    fn tst_log2() {
        assert_eq!(log2(1), 0);
        for n in 2..=3 {
            assert_eq!(log2(n), 1);
        }
        for n in 4..=7 {
            assert_eq!(log2(n), 2);
        }
    }
}
