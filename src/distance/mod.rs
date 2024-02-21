/// Distance based on spatial momentum components
pub mod eucl;

pub use eucl::EuclWithScaledPt;

use crate::event::Event;

use noisy_float::prelude::*;

/// A metric (distance function) in the space of all events
pub trait Distance<E = Event> {
    /// Compute the distance between two events
    fn distance(&self, ev1: &E, ev2: &E) -> N64;
}

impl<D, E> Distance<E> for &D
where
    D: Distance<E>,
{
    fn distance(&self, ev1: &E, ev2: &E) -> N64 {
        (*self).distance(ev1, ev2)
    }
}

/// Wrapper around distances storing also the events
#[derive(Debug)]
pub struct DistWrapper<'a, 'b, D: Distance> {
    ev_dist: &'a D,
    events: &'b [Event],
}

impl<'a, 'b, D: Distance> DistWrapper<'a, 'b, D> {
    /// Construct a distance wrapper
    pub fn new(ev_dist: &'a D, events: &'b [Event]) -> Self {
        Self { ev_dist, events }
    }
}

impl<D: Distance> Distance<usize> for DistWrapper<'_, '_, D> {
    fn distance(&self, e1: &usize, e2: &usize) -> N64 {
        self.ev_dist.distance(&self.events[*e1], &self.events[*e2])
    }
}
