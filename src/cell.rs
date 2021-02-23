use crate::event::Event;

use std::cmp::Ordering;

use noisy_float::prelude::*;
use rayon::prelude::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Default)]
pub struct Cell {
    events: Vec<Event>,
    radius: N64,
    weight_sum: N64,
}

impl Cell {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_seed(seed: Event) -> Self {
        let weight_sum = seed.weight;
        Self {
            events: vec![seed],
            radius: n64(0.),
            weight_sum,
        }
    }

    pub fn push(&mut self, event: Event) {
        self.weight_sum += event.weight;
        self.events.push(event);
    }

    pub fn push_with_dist(&mut self, event: Event, distance: N64) {
        self.push(event);
        self.radius = distance;
    }

    pub fn resample(&mut self) {
        let orig_weight_sum = self.weight_sum();
        match orig_weight_sum.cmp(&n64(0.)) {
            Ordering::Less => {}
            Ordering::Equal => {
                for event in &mut self.events {
                    event.weight = n64(0.);
                }
            }
            Ordering::Greater => {
                for event in &mut self.events {
                    event.weight = event.weight.abs();
                }
                let abs_weight_sum: N64 =
                    self.events.iter().map(|e| e.weight).sum();
                for event in &mut self.events {
                    event.weight *= orig_weight_sum / abs_weight_sum;
                }
            }
        }
    }

    pub fn nmembers(&self) -> usize {
        self.events.len()
    }

    pub fn radius(&self) -> N64 {
        self.radius
    }

    pub fn weight_sum(&self) -> N64 {
        self.weight_sum
    }

    pub fn iter(&self) -> std::slice::Iter<Event> {
        self.events.iter()
    }

    pub fn par_iter(&self) -> rayon::slice::Iter<Event> {
        self.events.par_iter()
    }
}

impl std::convert::From<Cell> for Vec<Event> {
    fn from(cell: Cell) -> Self {
        cell.events
    }
}
