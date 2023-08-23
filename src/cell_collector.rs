use std::collections::{BTreeMap, HashMap};

use crate::cell::Cell;

use log::info;
use noisy_float::prelude::*;
use rand::{
    distributions::{Distribution, Uniform},
    Rng,
};

const NCELLS: usize = 10;

/// Collect a number of cells of interest
#[derive(Default, Clone, Debug)]
pub struct CellCollector {
    first: Vec<(usize, Vec<usize>)>,
    random: Vec<(usize, Vec<usize>)>,
    // we keep the cell number as part of the key so that
    // different cells with the same number of members can be kept
    largest_by_radius: BTreeMap<(N64, usize), Vec<usize>>,
    largest_by_members: BTreeMap<(usize, usize), Vec<usize>>,
    largest_by_weight: BTreeMap<(N64, usize), Vec<usize>>,
    count: usize,
}

impl CellCollector {
    /// Constructor
    pub fn new() -> Self {
        Self {
            first: Vec::with_capacity(NCELLS),
            random: Vec::with_capacity(NCELLS),
            largest_by_radius: BTreeMap::new(),
            largest_by_members: BTreeMap::new(),
            largest_by_weight: BTreeMap::new(),
            count: 0,
        }
    }

    /// Maybe collect a cell
    ///
    /// Some of the cell characteristics are saved if any of the
    /// following criteria is fulfilled:
    ///
    /// 1. The cell radius is among the largest ones encountered so far
    /// 2. The number of cell members is among the largest ones
    ///    encountered so far
    /// 3. The cell weight is among the largest ones encountered so far
    /// 4. The cell is lucky: There is a 1/`N' chance to be saved,
    ///    where `N` is the number of events considered so far
    pub fn collect<R: Rng>(&mut self, cell: &Cell, mut rng: R) {
        let count = self.count;
        let r = cell.radius();
        let nmembers = cell.nmembers();
        let weight = cell.weight_sum();
        if count < NCELLS {
            self.first
                .push((count, cell.iter().map(|e| e.id()).collect()));
            self.random
                .push((count, cell.iter().map(|e| e.id()).collect()));
            self.largest_by_radius
                .insert((r, count), cell.iter().map(|e| e.id()).collect());
            self.largest_by_members.insert(
                (nmembers, count),
                cell.iter().map(|e| e.id()).collect(),
            );
            self.largest_by_weight
                .insert((weight, count), cell.iter().map(|e| e.id()).collect());
        } else {
            let (smallest_r, n) =
                *self.largest_by_radius.keys().next().unwrap();
            if r > smallest_r {
                self.largest_by_radius.remove(&(smallest_r, n)).unwrap();
                let events = cell.iter().map(|e| e.id()).collect();
                self.largest_by_radius.insert((r, count), events);
            }
            let (least_members, n) =
                *self.largest_by_members.keys().next().unwrap();
            if nmembers > least_members {
                self.largest_by_members.remove(&(least_members, n)).unwrap();
                let events = cell.iter().map(|e| e.id()).collect();
                self.largest_by_members.insert((nmembers, count), events);
            }
            let (smallest_weight, n) =
                *self.largest_by_weight.keys().next().unwrap();
            if weight > smallest_weight {
                self.largest_by_weight
                    .remove(&(smallest_weight, n))
                    .unwrap();
                let events = cell.iter().map(|e| e.id()).collect();
                self.largest_by_weight.insert((weight, count), events);
            }
            let distr = Uniform::from(0..=count);
            let idx = distr.sample(&mut rng);
            if idx < self.random.len() {
                let events = cell.iter().map(|e| e.id()).collect();
                self.random[idx] = (count, events);
            }
        }
        self.count += 1;
    }

    /// Write information on collected events to log at `info` level
    pub fn dump_info(&self) {
        info!("Cells by creation order:");
        for (id, events) in &self.first {
            info!("Cell {} with {} events", id, events.len());
        }
        info!("Largest cells by radius:");
        for ((r, id), events) in self.largest_by_radius.iter().rev() {
            info!("Cell {} with {} events and radius {}", id, events.len(), r);
        }
        info!("Largest cells by number of events:");
        for ((_, id), events) in self.largest_by_members.iter().rev() {
            info!("Cell {} with {} events", id, events.len());
        }
        info!("Cells with largest accumulated weights:");
        for ((weight, id), events) in self.largest_by_weight.iter().rev() {
            info!(
                "Cell {} with {} events and weight {:e}",
                id,
                events.len(),
                weight
            );
        }
        info!("Randomly selected cells:");
        for (id, events) in &self.random {
            info!("Cell {} with {} events", id, events.len());
        }
    }

    /// Return the saved cells
    ///
    /// The keys in the returned HashMap are the cell numbers,
    /// the corresponding values the event ids.
    pub fn event_cells(&self) -> HashMap<usize, Vec<usize>> {
        let mut result: HashMap<usize, Vec<_>> = HashMap::new();
        let all_cells = self
            .first
            .iter()
            .map(|(id, events)| (*id, events))
            .chain(self.random.iter().map(|(id, events)| (*id, events)))
            .chain(
                self.largest_by_radius
                    .iter()
                    .map(|((_r, id), events)| (*id, events)),
            )
            .chain(
                self.largest_by_members
                    .iter()
                    .map(|((_n, id), events)| (*id, events)),
            )
            .chain(
                self.largest_by_weight
                    .iter()
                    .map(|((_w, id), events)| (*id, events)),
            );
        for (cell, event_ids) in all_cells {
            for event_id in event_ids {
                result.entry(*event_id).or_default().push(cell)
            }
        }
        result
    }

    /// Combine two cell collectors into a single one
    pub fn combine(mut self, other: Self, rng: &mut impl Rng) -> Self {
        info!(
            "combining {} + {} cell observations",
            self.count, other.count
        );
        self.largest_by_members.extend(
            other
                .largest_by_members
                .into_iter()
                .map(|((n, id), ev)| ((n, id + self.count), ev)),
        );
        truncate(&mut self.largest_by_members, NCELLS);
        self.largest_by_radius.extend(
            other
                .largest_by_radius
                .into_iter()
                .map(|((r, id), ev)| ((r, id + self.count), ev)),
        );
        truncate(&mut self.largest_by_radius, NCELLS);
        self.largest_by_weight.extend(
            other
                .largest_by_weight
                .into_iter()
                .map(|((w, id), ev)| ((w, id + self.count), ev)),
        );
        truncate(&mut self.largest_by_weight, NCELLS);
        self.random.extend(
            other
                .random
                .into_iter()
                .map(|(id, ev)| (id + self.count, ev)),
        );
        while self.random.len() > NCELLS {
            let distr = Uniform::from(0..self.random.len());
            let idx = distr.sample(rng);
            self.random.swap_remove(idx);
        }
        self.count += other.count;
        self
    }
}

fn truncate<K: Ord, V>(map: &mut BTreeMap<K, V>, n: usize) {
    let mut count = 0;
    map.retain(|_, _| {
        count += 1;
        count <= n
    });
}
