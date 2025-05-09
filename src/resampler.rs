use std::cell::RefCell;
use std::default::Default;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::cell::Cell;
use crate::cell_collector::CellCollector;
use crate::distance::{DistWrapper, Distance, EuclWithScaledPt};
use crate::event::Event;
use crate::neighbour_search::TreeSearch;
use crate::progress_bar::{Progress, ProgressBar};
use crate::seeds::{StrategicSelector, Strategy};
use crate::traits::{
    NeighbourSearch, NeighbourSearchAlgo, ObserveCell, Resample, SelectSeeds,
};

use log::{debug, info, trace};
use noisy_float::prelude::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rayon::prelude::*;
use thiserror::Error;
use thread_local::ThreadLocal;

/// Error during resampling
#[derive(Debug, Error)]
pub enum ResamplingError {}

/// Main resampling class
pub struct Resampler<D, N, O, S> {
    seeds: S,
    distance: D,
    neighbour_search: PhantomData<N>,
    observer: O,
    max_cell_size: Option<f64>,
}

impl<D, N, O, S> Resampler<D, N, O, S> {
    fn print_wt_sum(&self, events: &[Event]) {
        let sum_wt: N64 = events.iter().map(|e| e.weight()).sum();
        let sum_wtsqr: N64 =
            events.iter().map(|e| e.weight() * e.weight()).sum();
        let sum_neg_wt: N64 =
            events.iter().map(|e| e.weight()).filter(|&w| w < 0.).sum();
        info!(
            "Initial sum of weights: {sum_wt:.3e} ± {:.3e}",
            sum_wtsqr.sqrt()
        );
        info!(
            "Negative weight fraction: {:.3}",
            -sum_neg_wt / (sum_wt - sum_neg_wt * 2.)
        );
    }
}

impl<D, N, O, S, T> Resample for Resampler<D, N, O, S>
where
    D: Distance + Send + Sync,
    N: NeighbourSearchAlgo,
    for<'x, 'y, 'z> &'x <N as NeighbourSearchAlgo>::Output<DistWrapper<'y, 'z, D>>: NeighbourSearch + Send + Sync,
    for<'x, 'y, 'z> <&'x <N as NeighbourSearchAlgo>::Output<DistWrapper<'y, 'z, D>> as NeighbourSearch>::Iter: Iterator<Item = (usize, N64)>,
    S: SelectSeeds<ParallelIter = T> + Send + Sync,
    T: ParallelIterator<Item = usize>,
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
        events: &[Event],
    ) -> Result<(), Self::Error> {
        {
            self.print_wt_sum(&events);

            let nneg_weight = events.iter().filter(|e| e.weight() < 0.).count();

            let max_cell_size = n64(self.max_cell_size.unwrap_or(f64::MAX));

            info!("Initialising nearest-neighbour search");
            let neighbour_search = N::new_with_dist(
                events.len(),
                DistWrapper::new(&self.distance, &events),
                max_cell_size,
            );

            info!("Resampling {nneg_weight} cells");
            let progress = ProgressBar::new(nneg_weight as u64, "events treated:");
            let seeds = self.seeds.select_seeds(&events);
            seeds.for_each(|seed| {
                assert!(seed < events.len());
                if events[seed].weight() > 0. {
                    return;
                }
                trace!("New cell around event {}", events[seed].id());
                let mut cell =
                    Cell::new(&events, seed, &neighbour_search);
                cell.resample();
                self.observer.observe_cell(&cell);
                progress.inc(1);
            });
            progress.finish();
            debug!("Combining cell observations");
            self.observer.finish();

            debug!("Resampling done");
        }
        Ok(())
    }
}

/// Construct a `Resampler` object
pub struct ResamplerBuilder<D, O, S, N = TreeSearch> {
    seeds: S,
    distance: D,
    neighbour_search: PhantomData<N>,
    observer: O,
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
            max_cell_size: self.max_cell_size,
        }
    }

    /// Define how and in which order to choose cell seeds
    pub fn seeds<SS, T>(self, seeds: SS) -> ResamplerBuilder<D, O, SS, N>
    where
        SS: SelectSeeds<ParallelIter = T>,
        T: ParallelIterator<Item = usize>,
    {
        ResamplerBuilder {
            seeds,
            distance: self.distance,
            neighbour_search: PhantomData,
            observer: self.observer,
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
            max_cell_size: self.max_cell_size,
        }
    }

    /// Algorithm for nearest-neighbour search
    pub fn neighbour_search<NN>(self) -> ResamplerBuilder<D, O, S, NN> {
        ResamplerBuilder {
            seeds: self.seeds,
            distance: self.distance,
            neighbour_search: PhantomData,
            observer: self.observer,
            max_cell_size: self.max_cell_size,
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

impl Default
    for ResamplerBuilder<
        EuclWithScaledPt,
        NoObserver,
        StrategicSelector,
        TreeSearch,
    >
{
    fn default() -> Self {
        Self {
            seeds: Default::default(),
            distance: Default::default(),
            neighbour_search: PhantomData,
            observer: Default::default(),
            max_cell_size: Default::default(),
        }
    }
}

/// Default implementation of cell resampling
pub struct DefaultResampler<N> {
    ptweight: f64,
    strategy: Strategy,
    max_cell_size: Option<f64>,
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    neighbour_search: PhantomData<N>,
}

type ResampleHelper<N> =
    Resampler<EuclWithScaledPt, N, Observer, StrategicSelector>;

impl<N> Resample for DefaultResampler<N>
where
    ResampleHelper<N>: Resample,
    ResamplingError: From<<ResampleHelper<N> as Resample>::Error>,
{
    type Error = ResamplingError;

    fn resample(&mut self, events: &[Event]) -> Result<(), Self::Error> {
        let observer_data = ObserverData {
            cell_collector: self
                .cell_collector
                .clone()
                .map(|c| c.borrow().clone()),
            ..Default::default()
        };
        let observer = Observer {
            central: observer_data,
            threaded: Default::default(),
        };

        let mut resampler = ResamplerBuilder::default()
            .seeds(StrategicSelector::new(self.strategy))
            .distance(EuclWithScaledPt::new(n64(self.ptweight)))
            .max_cell_size(self.max_cell_size)
            .observer(observer)
            .neighbour_search::<N>()
            .build();
        crate::traits::Resample::resample(&mut resampler, events)?;

        if let Some(c) = self.cell_collector.as_mut() {
            c.replace(resampler.observer.central.cell_collector.unwrap());
        }
        Ok(())
    }
}

impl<N> DefaultResampler<N> {
    /// Get the callback upon cell construction
    pub fn cell_collector(&self) -> Option<Rc<RefCell<CellCollector>>> {
        self.cell_collector.as_ref().cloned()
    }
}

/// Build a [DefaultResampler]
pub struct DefaultResamplerBuilder<N> {
    ptweight: f64,
    strategy: Strategy,
    max_cell_size: Option<f64>,
    cell_collector: Option<Rc<RefCell<CellCollector>>>,
    neighbour_search: PhantomData<N>,
}

impl Default for DefaultResamplerBuilder<TreeSearch> {
    fn default() -> Self {
        Self {
            ptweight: 0.,
            strategy: Strategy::default(),
            max_cell_size: None,
            cell_collector: None,
            neighbour_search: PhantomData,
        }
    }
}

impl<N> DefaultResamplerBuilder<N> {
    /// Set the τ factor in the distance measure
    pub fn ptweight(mut self, value: f64) -> Self {
        self.ptweight = value;
        self
    }

    /// Set the strategy for selecting cell seeds
    pub fn strategy(mut self, value: Strategy) -> Self {
        self.strategy = value;
        self
    }

    /// Set the maximum cell size
    pub fn max_cell_size(mut self, value: Option<f64>) -> Self {
        self.max_cell_size = value;
        self
    }

    /// Set a callback after cell construction
    pub fn cell_collector(
        mut self,
        value: Option<Rc<RefCell<CellCollector>>>,
    ) -> Self {
        self.cell_collector = value;
        self
    }

    /// Set the nearest neighbour search algorithm
    pub fn neighbour_search<NN>(self) -> DefaultResamplerBuilder<NN> {
        DefaultResamplerBuilder {
            ptweight: self.ptweight,
            strategy: self.strategy,
            max_cell_size: self.max_cell_size,
            cell_collector: self.cell_collector,
            neighbour_search: PhantomData,
        }
    }

    /// Build a [DefaultResampler]
    pub fn build(self) -> DefaultResampler<N> {
        DefaultResampler {
            ptweight: self.ptweight,
            strategy: self.strategy,
            max_cell_size: self.max_cell_size,
            cell_collector: self.cell_collector,
            neighbour_search: PhantomData,
        }
    }
}

fn median_radius(radii: &mut [N64]) -> N64 {
    radii.sort_unstable();
    radii[radii.len() / 2]
}

#[derive(Debug, Default)]
struct Observer {
    central: ObserverData,
    threaded: ThreadLocal<RefCell<ObserverData>>,
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
        let mut data = self
            .threaded
            .get_or(|| RefCell::new(self.central.clone()))
            .borrow_mut();
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
        let res = data
            .into_iter()
            .map(|c| c.into_inner())
            .reduce(|acc, c| acc.combine(c));
        if let Some(mut res) = res {
            info!("Created {} cells", res.cell_radii.len());
            if res.nneg > 0 {
                info!("{} cells had negative weight!", res.nneg);
            }
            info!(
                "Median radius: {:.3}",
                median_radius(res.cell_radii.as_mut_slice())
            );
            if let Some(c) = res.cell_collector.as_ref() {
                c.dump_info();
            }
            self.central = res;
        }
    }
}

impl ObserverData {
    pub fn combine(mut self, mut other: Self) -> Self {
        self.cell_radii.append(&mut other.cell_radii);
        self.nneg += other.nneg;
        self.cell_collector = match (self.cell_collector, other.cell_collector)
        {
            (Some(c1), Some(c2)) => Some(c1.combine(c2, &mut self.rng)),
            (Some(c), None) => Some(c),
            (None, Some(c)) => Some(c),
            (None, None) => None,
        };
        self
    }
}

/// Dummy callback for cell resampling (doing nothing)
#[derive(Copy, Clone, Default, Debug)]
pub struct NoObserver {}
impl ObserveCell for NoObserver {
    fn observe_cell(&self, _cell: &Cell) {}
}

/// Default cell observer doing nothing
pub const NO_OBSERVER: NoObserver = NoObserver {};

/// Logarithm in base 2, rounded down
pub const fn log2(n: u32) -> u32 {
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
