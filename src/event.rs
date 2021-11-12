use crate::four_vector::FourVector;

use std::default::Default;
use std::convert::From;

use noisy_float::prelude::*;

pub type MomentumSet = Vec<FourVector>;

/// Build and `Event'
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub struct EventBuilder {
    id: usize,
    weight: N64,

    outgoing_by_pid: Vec<(i32, FourVector)>,
}

impl EventBuilder {
    /// New event with the given `id`, vanishing weight and no particles
    pub fn new(id: usize) -> Self {
        Self {
            id,
            weight: n64(0.),
            outgoing_by_pid: Vec::new()
        }
    }

    /// New event with the given `id`, and space reserved for the given number of particles
    pub fn with_capacity(id: usize, cap: usize) -> Self {
        Self {
            id,
            weight: n64(0.),
            outgoing_by_pid: Vec::with_capacity(cap)
        }
    }

    /// Add an outgoing particle with particle id `pid' and four-momentum `p'
    ///
    /// The particle id should follow the
    /// [PDG Monte Carlo Particle Numbering Scheme](https://pdg.lbl.gov/2021/mcdata/mc_particle_id_contents.html)
    pub fn add_outgoing(&mut self, pid: i32, p: FourVector) -> &mut Self {
        self.outgoing_by_pid.push((pid, p));
        self
    }

    /// Set the event weight
    pub fn weight(&mut self, weight: N64) -> &mut Self {
        self.weight = weight;
        self
    }

    /// Construct an event
    pub fn build(self) -> Event {
        let outgoing_by_pid = compress_outgoing(self.outgoing_by_pid);
        Event {
            id: self.id,
            weight: self.weight,
            outgoing_by_pid
        }
    }
}

impl From<EventBuilder> for Event {
    fn from(b: EventBuilder) -> Self {
        b.build()
    }
}

fn compress_outgoing(mut out: Vec<(i32, FourVector)>) -> Vec<(i32, Vec<FourVector>)> {
    out.sort_unstable_by(|a, b| b.cmp(a));
    let mut outgoing_by_pid : Vec<(i32, Vec<_>)> = Vec::new();
    for (id, p) in out {
        match outgoing_by_pid.last_mut() {
            Some((pid, v)) if *pid == id => v.push(p),
            _ => outgoing_by_pid.push((id, vec![p]))
        }
    }
    outgoing_by_pid
}

/// A Monte Carlo scattering event
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Default)]
pub struct Event {
    id: usize,
    pub weight: N64,

    outgoing_by_pid: Vec<(i32, MomentumSet)>,
}

const EMPTY_SLICE: &[FourVector] = &[];

impl Event {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the event id
    pub fn id(&self) -> usize {
        self.id
    }

    /// Access the outgoing particle momenta grouped by particle id
    pub fn outgoing(&self) -> &[(i32, MomentumSet)] {
        self.outgoing_by_pid.as_slice()
    }

    /// Access the outgoing particle momenta with the given particle id
    pub fn outgoing_with_pid(&self, pid: i32) -> &[FourVector] {
        let idx = self.outgoing_by_pid.binary_search_by(|probe| pid.cmp(&probe.0));
        if let Ok(idx) = idx {
            &self.outgoing_by_pid[idx].1
        } else {
            EMPTY_SLICE
        }
    }

    /// Extract the outgoing particle momenta grouped by particle id
    pub fn into_outgoing(self) -> Vec<(i32, MomentumSet)> {
        self.outgoing_by_pid
    }
}
