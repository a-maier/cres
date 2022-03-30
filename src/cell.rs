use crate::distance::Distance;
use crate::event::Event;
use crate::traits::NeighbourSearch;

use log::{debug, trace};
use noisy_float::prelude::*;

/// A cell
///
/// See [arXiv:2109.07851](https://arxiv.org/abs/2109.07851) for details
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Cell<'a> {
    events: &'a mut [Event],
    members: Vec<usize>,
    radius: N64,
    weight_sum: N64,
}

/// Construct a new cell
///
/// The `events` items have the form (N64, Event), where
/// the first tuple element is used to store distances
impl<'a> Cell<'a> {
    pub fn new<'b: 'a, 'c, F: Distance + Sync + Send, N>(
        events: &'b mut [Event],
        seed_idx: usize,
        distance: &F,
        neighbour_search: N,
        max_size: N64,
    ) -> Self
    where
        N: NeighbourSearch,
    <N as NeighbourSearch>::Iter: Iterator<Item=(usize, N64)>,
    {
        let mut weight_sum = events[seed_idx].weight;
        debug_assert!(weight_sum < 0.);
        debug!("Cell seed with weight {:e}", weight_sum);
        let mut members = vec![seed_idx];
        let mut radius = n64(0.);

        let neighbours = neighbour_search.nearest_in(
            &seed_idx,
            |&i, &j| distance.distance(&events[i], &events[j])
        );

        for (next_idx, dist) in neighbours {
            if dist > max_size {
                break;
            }
            trace!(
                "adding event with distance {dist}, weight {:e} to cell",
                events[next_idx].weight
            );
            weight_sum += events[next_idx].weight;
            members.push(next_idx);
            radius = dist;
            if weight_sum >= 0. {
                break;
            }
        }
        Self {
            events,
            members,
            weight_sum,
            radius,
        }
    }

    /// Resample
    ///
    /// This redistributes weights in such a way that all weights have
    /// the same sign.
    pub fn resample(&mut self) {
        let orig_weight_sum = self.weight_sum();
        if orig_weight_sum == n64(0.) {
            for &idx in &self.members {
                self.events[idx].weight = n64(0.);
            }
        } else {
            let mut abs_weight_sum = n64(0.);
            for &idx in &self.members {
                let awt = self.events[idx].weight.abs();
                self.events[idx].weight = awt;
                abs_weight_sum += awt;
            }
            for &idx in &self.members {
                self.events[idx].weight *= orig_weight_sum / abs_weight_sum;
            }
        }
    }

    /// Number of events in cell
    pub fn nmembers(&self) -> usize {
        self.members.len()
    }

    /// Number of negative-weight events in cell
    pub fn nneg_weights(&self) -> usize {
        self.members
            .iter()
            .filter(|&&idx| self.events[idx].weight < 0.)
            .count()
    }

    /// Cell radius
    ///
    /// This is the largest distance from the seed to any event in the cell.
    pub fn radius(&self) -> N64 {
        self.radius
    }

    /// Sum of event weights inside the cell
    pub fn weight_sum(&self) -> N64 {
        self.weight_sum
    }

    /// Iterator over (distance, cell member)
    pub fn iter(
        &'a self,
    ) -> impl std::iter::Iterator<Item = &'a Event> + 'a {
        self.members.iter().map(move |idx| &self.events[*idx])
    }
}
