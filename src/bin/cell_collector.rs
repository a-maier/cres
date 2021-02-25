use std::collections::{BTreeMap, HashMap};

use cres::cell::Cell;

use noisy_float::prelude::*;
use log::info;
use rand::{
    distributions::{Distribution, Uniform},
    prelude::SliceRandom,
    Rng,
};

const NCELLS: usize = 10;

#[derive(Default, Clone, Debug)]
pub(crate) struct CellCollector {
    first: Vec<(usize, Vec<usize>)>,
    random: Vec<(usize, Vec<usize>)>,
    largest_by_radius: BTreeMap<N64, (usize, Vec<usize>)>,
    largest_by_members:  BTreeMap<usize, (usize, Vec<usize>)>,
    largest_by_weight:  BTreeMap<N64, (usize, Vec<usize>)>,
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

    pub fn collect<R: Rng>(
        &mut self,
        cell: &Cell,
        mut rng: R
    ) {
        let count = self.count;
        let r = cell.radius();
        let nmembers = cell.nmembers();
        let weight = cell.weight_sum();
        if count < NCELLS {
            let events = cell.iter().map(|(_d, e)| e.id);
            self.first.push((count, events.clone().collect()));
            self.random.push((count, events.clone().collect()));
            self.largest_by_radius.insert(r, (count, events.clone().collect()));
            self.largest_by_members.insert(nmembers, (count, events.clone().collect()));
            self.largest_by_weight.insert(weight, (count, events.collect()));
        } else {
            let smallest_r = *self.largest_by_radius.keys().next().unwrap();
            if r > smallest_r {
                self.largest_by_radius.remove(&smallest_r).unwrap();
                let events = cell.iter().map(|(_d, e)| e.id).collect();
                self.largest_by_radius.insert(r, (count, events));
            }
            let least_members = *self.largest_by_members.keys().next().unwrap();
            if nmembers > least_members {
                self.largest_by_members.remove(&least_members).unwrap();
                let events = cell.iter().map(|(_d, e)| e.id).collect();
                self.largest_by_members.insert(nmembers, (count, events));
            }
            let smallest_weight = *self.largest_by_weight.keys().next().unwrap();
            if weight > smallest_weight {
                self.largest_by_weight.remove(&smallest_weight).unwrap();
                let events = cell.iter().map(|(_d, e)| e.id).collect();
                self.largest_by_weight.insert(weight, (count, events));
            }
            let distr = Uniform::from(0..=count + 1);
            if distr.sample(&mut rng) == count + 1 {
                let events = cell.iter().map(|(_d, e)| e.id).collect();
                *self.random.choose_mut(&mut rng).unwrap() = (count, events)
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
        for (r, (id, events)) in self.largest_by_radius.iter().rev() {
            info!("Cell {} with {} events and radius {}", id, events.len(), r);
        }
        info!("Largest cells by number of events:");
        for (_, (id, events)) in self.largest_by_members.iter().rev() {
            info!("Cell {} with {} events", id, events.len());
        }
        info!("Cells with largest accumulated weights:");
        for (weight, (id, events)) in self.largest_by_weight.iter().rev() {
            info!("Cell {} with {} events and weight {:e}", id, events.len(), weight);
        }
        info!("Randomly selected cells:");
        for (id, events) in &self.random {
            info!("Cell {} with {} events", id, events.len());
        }
    }

    pub fn event_cells(&self) -> HashMap<usize, Vec<usize>> {
        let mut result: HashMap<usize, Vec<_>> = HashMap::new();
        let all_cells = self.first.iter()
            .chain(self.random.iter())
            .chain(self.largest_by_radius.iter().map(|(_r, id)| id))
            .chain(self.largest_by_members.iter().map(|(_n, id)| id))
            .chain(self.largest_by_weight.iter().map(|(_n, id)| id));
        for (cell, event_ids) in  all_cells {
            for event_id in event_ids {
                result.entry(*event_id).or_default().push(*cell)
            }
        }
        result
    }
}
