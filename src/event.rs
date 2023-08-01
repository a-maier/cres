use crate::four_vector::FourVector;

use std::convert::From;
use std::default::Default;

use noisy_float::prelude::*;
use particle_id::ParticleID;

pub type MomentumSet = Box<[FourVector]>;

#[cfg(feature = "multiweight")]
type BuilderWeights = Vec<N64>;
#[cfg(feature = "multiweight")]
type Weights = Box<[N64]>;
#[cfg(not(feature = "multiweight"))]
type BuilderWeights = N64;
#[cfg(not(feature = "multiweight"))]
type Weights = N64;

/// Build an [Event]
#[derive(PartialEq, Eq, PartialOrd, Ord, Default, Debug, Clone)]
pub struct EventBuilder {
    weights: BuilderWeights,

    outgoing_by_pid: Vec<(ParticleID, FourVector)>,
}

impl EventBuilder {
    /// New event without weights or particles
    pub fn new() -> Self {
        Self {
            weights: Default::default(),
            outgoing_by_pid: Vec::new(),
        }
    }

    /// New event with space reserved for the given number of particles
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            weights: Default::default(),
            outgoing_by_pid: Vec::with_capacity(cap),
        }
    }

    /// Add an outgoing particle with particle id `pid' and four-momentum `p'
    ///
    /// The particle id should follow the
    /// [PDG Monte Carlo Particle Numbering Scheme](https://pdg.lbl.gov/2021/mcdata/mc_particle_id_contents.html)
    pub fn add_outgoing(&mut self, pid: ParticleID, p: FourVector) -> &mut Self {
        self.outgoing_by_pid.push((pid, p));
        self
    }

    /// Set the event weights
    pub fn weights(&mut self, weights: BuilderWeights) -> &mut Self {
        self.weights = weights;
        self
    }

    /// Construct an event
    pub fn build(self) -> Event {
        let outgoing_by_pid = compress_outgoing(self.outgoing_by_pid);
        Event {
            id: Default::default(),
            #[cfg(feature = "multiweight")]
            weights: self.weights.into_boxed_slice(),
            #[cfg(not(feature = "multiweight"))]
            weights: self.weights,
            outgoing_by_pid,
        }
    }
}

impl From<EventBuilder> for Event {
    fn from(b: EventBuilder) -> Self {
        b.build()
    }
}

fn compress_outgoing(
    mut out: Vec<(ParticleID, FourVector)>,
) -> Box<[(ParticleID, MomentumSet)]> {
    out.sort_unstable_by(|a, b| b.cmp(a));
    let mut outgoing_by_pid: Vec<(ParticleID, Vec<_>)> = Vec::new();
    for (id, p) in out {
        match outgoing_by_pid.last_mut() {
            Some((pid, v)) if *pid == id => v.push(p),
            _ => outgoing_by_pid.push((id, vec![p])),
        }
    }
    let outgoing_by_pid = Vec::from_iter(
        outgoing_by_pid.into_iter()
            .map(|(id, p)| (id, p.into_boxed_slice()))
    );
    outgoing_by_pid.into_boxed_slice()
}

/// A Monte Carlo scattering event
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Default)]
pub struct Event {
    pub id: usize,
    pub weights: Weights,

    outgoing_by_pid: Box<[(ParticleID, MomentumSet)]>,
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
    pub fn outgoing(&self) -> &[(ParticleID, MomentumSet)] {
        &self.outgoing_by_pid
    }

    /// Access the outgoing particle momenta with the given particle id
    pub fn outgoing_with_pid(&self, pid: ParticleID) -> &[FourVector] {
        let idx = self
            .outgoing_by_pid
            .binary_search_by(|probe| pid.cmp(&probe.0));
        if let Ok(idx) = idx {
            &self.outgoing_by_pid[idx].1
        } else {
            EMPTY_SLICE
        }
    }

    /// The central event weight
    pub fn weight(&self) -> N64 {
        #[cfg(feature = "multiweight")]
        return self.weights[0];

        #[cfg(not(feature = "multiweight"))]
        self.weights
    }

    /// Extract the outgoing particle momenta grouped by particle id
    pub fn into_outgoing(self) -> Box<[(ParticleID, MomentumSet)]> {
        self.outgoing_by_pid
    }

    /// Number of weights
    pub fn n_weights(&self) -> usize {
        #[cfg(feature = "multiweight")]
        return self.weights.len();

        #[cfg(not(feature = "multiweight"))]
        1
    }

    /// Rescale weights by some factor
    pub fn rescale_weights(&mut self, scale: N64) {
        #[cfg(feature = "multiweight")]
        for wt in self.weights.iter_mut() {
            *wt *= scale
        }
        #[cfg(not(feature = "multiweight"))]
        {
            self.weights *= scale;
        }
    }
}
