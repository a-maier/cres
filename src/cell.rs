use crate::event::Event;

use std::cmp::Ordering;

use noisy_float::prelude::*;
use rayon::prelude::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Cell<'a> {
    events: &'a mut [(N64, Event)],
    radius: N64,
    weight_sum: N64,
}

impl<'a> Cell<'a> {
    pub fn new<'b: 'a>(events: &'b mut [(N64, Event)]) -> Self {
        events.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        Self::new_unchecked(events)
    }

    pub fn new_unchecked<'b: 'a>(
        events: &'b mut [(N64, Event)]
    ) -> Self {
        // TODO: add as soon as `is_sorted_by` is stable
        // debug_assert!(events.is_sorted_by(|a, b| b.0.cmp(&a.0)));
        let weight_sum = events.iter().map(|(_d, e)| e.weight).sum();
        Self::with_weight_sum_unchecked(events, weight_sum)
    }

    pub fn with_weight_sum_unchecked<'b: 'a>(
        events: &'b mut [(N64, Event)],
        weight_sum: N64,
    ) -> Self {
        let radius = events.first().unwrap().0;
        Self{events, radius, weight_sum}
    }

    pub fn resample(&mut self) {
        let orig_weight_sum = self.weight_sum();
        match orig_weight_sum.cmp(&n64(0.)) {
            Ordering::Less => {}
            Ordering::Equal => {
                for event in self.events.iter_mut() {
                    event.1.weight = n64(0.);
                }
            }
            Ordering::Greater => {
                for event in self.events.iter_mut() {
                    event.1.weight = event.1.weight.abs();
                }
                let abs_weight_sum: N64 =
                    self.events.iter().map(|e| e.1.weight).sum();
                for event in self.events.iter_mut() {
                    event.1.weight *= orig_weight_sum / abs_weight_sum;
                }
            }
        }
    }

    pub fn nmembers(&self) -> usize {
        self.events.len()
    }

    pub fn nneg_weights(&self) -> usize {
        self.events.iter().filter(|e| e.1.weight < 0.).count()
    }

    pub fn radius(&self) -> N64 {
        self.radius
    }

    pub fn weight_sum(&self) -> N64 {
        self.weight_sum
    }

    pub fn iter(&self) -> std::slice::Iter<(N64, Event)> {
        self.events.iter()
    }

    pub fn par_iter(&self) -> rayon::slice::Iter<(N64, Event)> {
        self.events.par_iter()
    }
}
