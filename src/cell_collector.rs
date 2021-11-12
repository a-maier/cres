use std::collections::{BTreeMap, HashMap};

use crate::cell::Cell;

use log::info;
use noisy_float::prelude::*;
use rand::{
    distributions::{Distribution, Uniform},
    Rng,
};

const NCELLS: usize = 10;

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

    pub fn collect<R: Rng>(&mut self, cell: &Cell, mut rng: R) {
        let count = self.count;
        let r = cell.radius();
        let nmembers = cell.nmembers();
        let weight = cell.weight_sum();
        if count < NCELLS {
            self.first
                .push((count, cell.iter().map(|(_d, e)| e.id()).collect()));
            self.random
                .push((count, cell.iter().map(|(_d, e)| e.id()).collect()));
            self.largest_by_radius.insert(
                (r, count),
                cell.iter().map(|(_d, e)| e.id()).collect(),
            );
            self.largest_by_members.insert(
                (nmembers, count),
                cell.iter().map(|(_d, e)| e.id()).collect(),
            );
            self.largest_by_weight.insert(
                (weight, count),
                cell.iter().map(|(_d, e)| e.id()).collect(),
            );
        } else {
            let (smallest_r, n) =
                *self.largest_by_radius.keys().next().unwrap();
            if r > smallest_r {
                self.largest_by_radius.remove(&(smallest_r, n)).unwrap();
                let events = cell.iter().map(|(_d, e)| e.id()).collect();
                self.largest_by_radius.insert((r, count), events);
            }
            let (least_members, n) =
                *self.largest_by_members.keys().next().unwrap();
            if nmembers > least_members {
                self.largest_by_members.remove(&(least_members, n)).unwrap();
                let events = cell.iter().map(|(_d, e)| e.id()).collect();
                self.largest_by_members.insert((nmembers, count), events);
            }
            let (smallest_weight, n) =
                *self.largest_by_weight.keys().next().unwrap();
            if weight > smallest_weight {
                self.largest_by_weight
                    .remove(&(smallest_weight, n))
                    .unwrap();
                let events = cell.iter().map(|(_d, e)| e.id()).collect();
                self.largest_by_weight.insert((weight, count), events);
            }
            let distr = Uniform::from(0..=count);
            let idx = distr.sample(&mut rng);
            if idx < self.random.len() {
                let events = cell.iter().map(|(_d, e)| e.id()).collect();
                self.random[idx] = (count, events);
            }
        }
        self.count += 1;
    }

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
}
