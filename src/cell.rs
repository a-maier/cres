use crate::event::Event;

use std::cmp::Ordering;

use noisy_float::prelude::*;
use rayon::prelude::*;
use log::{debug, trace};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Cell<'a> {
    events: &'a mut [(N64, Event)],
    radius: N64,
    weight_sum: N64,
}

impl<'a> Cell<'a> {
    pub fn new<'b: 'a, F>(
        events: &'b mut [(N64, Event)],
        distance: F
    ) -> Option<(Self, &'b mut [(N64, Event)])>
    where F: Sync + Fn(&Event, &Event) -> N64
    {
        let seed = events.par_iter().enumerate().min_by_key(|(_n, (_dist, e))| e.weight);
        if let Some((n, _)) = seed {
            Self::from_seed(events, n, distance)
        } else {
            None
        }
    }

    fn from_seed<'b: 'a, F>(
        events: &'b mut [(N64, Event)],
        seed_idx: usize,
        distance: F
    ) -> Option<(Self, &'b mut [(N64, Event)])>
    where F: Sync + Fn(&Event, &Event) -> N64
    {
        let mut weight_sum = events[seed_idx].1.weight;
        if weight_sum >= 0. {
            return None;
        }
        debug!("Cell seed with weight {:e}", weight_sum);
        let last_idx = events.len() - 1;
        events.swap(seed_idx, last_idx);
        let (mut seed, mut rest) = events.split_last_mut().unwrap();
        seed.0 = n64(0.);
        let seed = seed;

        rest.par_iter_mut().for_each(
            |(dist, e)| *dist = distance(e, &seed.1)
        );

        while weight_sum < 0. {
            let nearest = rest
                .par_iter()
                .enumerate()
                .min_by_key(|(_idx, (dist, _event))| dist);
            let nearest_idx = if let Some((idx, (dist, event))) = nearest {
                trace!(
                    "adding event with distance {}, weight {:e} to cell",
                    dist,
                    event.weight
                );
                weight_sum += event.weight;
                idx
            } else {
                break
            };
            rest.swap(nearest_idx, rest.len() - 1);
            let last_idx = rest.len() - 1;
            rest = &mut rest[..last_idx];
        }
        let rest_len = rest.len();
        let (rest, cell) = events.split_at_mut(rest_len);
        let radius = cell.first().unwrap().0;
        let cell = Self {
            events: cell,
            weight_sum,
            radius
        };
        Some((cell, rest))
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
