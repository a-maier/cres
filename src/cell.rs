use crate::event::{Event, Weights};
use crate::traits::NeighbourSearch;

use log::{debug, trace};
use noisy_float::prelude::*;
use parking_lot::RwLockWriteGuard;

/// A cell
///
/// See [arXiv:2109.07851](https://arxiv.org/abs/2109.07851) for details
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Cell<'a> {
    events: &'a [Event],
    members: Vec<usize>,
    radius: N64,
    weight_sum: N64,
}

impl<'a> Cell<'a> {
    /// Construct a new cell from the given `events` with
    /// `events[seed_idx]` as seed, distance measure `distance` and
    /// neighbour search implementation `neighbour_search`
    pub fn new<'b: 'a, 'c, N>(
        events: &'b [Event],
        seed_idx: usize,
        neighbour_search: N,
    ) -> Self
    where
        N: NeighbourSearch,
        <N as NeighbourSearch>::Iter: Iterator<Item = (usize, N64)>,
    {
        let mut weight_sum = events[seed_idx].weight();
        debug_assert!(weight_sum < 0.);
        debug!("Cell seed {seed_idx}  with weight {:e}", weight_sum);
        let mut members = vec![seed_idx];
        let mut radius = n64(0.);

        let neighbours = neighbour_search.nearest(&seed_idx);

        for (next_idx, dist) in neighbours {
            trace!(
                "adding event {next_idx} with distance {dist}, weight {:e} to cell",
                events[next_idx].weight()
            );
            weight_sum += events[next_idx].weight();
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
    ///
    /// The current implementation sets all weights to the mean weight
    /// over the cell.
    #[cfg(feature = "multiweight")]
    pub fn resample(&mut self) {
        use std::ops::{Deref, DerefMut};

        self.members.sort_unstable(); // sort to prevent deadlocks
        let mut member_weights = self.write_lock_weights();
        let (first, rest) = member_weights.split_first_mut().unwrap();

        let mut avg_wts = std::mem::take(first.deref_mut());
        for idx in rest.iter() {
            avg_wts += idx.deref();
        }
        let inv_norm = n64(1. / self.nmembers() as f64);
        for wt in avg_wts.iter_mut() {
            *wt *= inv_norm;
        }
        for idx in rest {
            idx.copy_from(&avg_wts);
        }
        *first.deref_mut() = avg_wts;
    }

    /// Resample
    ///
    /// This redistributes weights in such a way that all weights have
    /// the same sign.
    ///
    /// The current implementation sets all weights to the mean weight
    /// over the cell.
    #[cfg(not(feature = "multiweight"))]
    pub fn resample(&mut self) {
        use std::ops::Deref;

        self.members.sort_unstable(); // sort to prevent deadlocks
        let member_weights = self.write_lock_weights();

        let total_wt: N64 =
            member_weights.iter().map(|w| w.deref().central()).sum();
        let avg_wt = total_wt / (self.nmembers() as f64);
        for mut wt in member_weights {
            *wt = Weights::new_single(avg_wt);
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
            .filter(|&&idx| self.events[idx].weight() < 0.)
            .count()
    }

    /// Cell radius
    ///
    /// This is the largest distance from the seed to any event in the cell.
    pub fn radius(&self) -> N64 {
        self.radius
    }

    /// Sum of central event weights inside the cell
    pub fn weight_sum(&self) -> N64 {
        self.weight_sum
    }

    /// Iterator over cell members
    pub fn iter(&'a self) -> impl std::iter::Iterator<Item = &'a Event> + 'a {
        self.members.iter().map(move |idx| &self.events[*idx])
    }

    fn write_lock_weights(&self) -> Vec<RwLockWriteGuard<'_, Weights>> {
        self.members
            .iter()
            .map(|i| self.events[*i].weights.write())
            .collect()
    }
}
