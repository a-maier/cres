use crate::four_vector::FourVector;

use std::convert::From;
use std::default::Default;
use std::ops::AddAssign;

use derivative::Derivative;
use noisy_float::prelude::*;
use parking_lot::RwLock;
use particle_id::ParticleID;
use serde::{Deserialize, Serialize};

/// Particle momenta
pub type MomentumSet = Box<[FourVector]>;

#[cfg(feature = "multiweight")]
type BuilderWeights = Vec<N64>;
#[cfg(not(feature = "multiweight"))]
type BuilderWeights = N64;

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
    pub fn add_outgoing(
        &mut self,
        pid: ParticleID,
        p: FourVector,
    ) -> &mut Self {
        self.outgoing_by_pid.push((pid, p));
        self
    }

    /// Add an event weight
    ///
    /// Overwrite the existing weight if the multiweight feature is disabled
    pub fn add_weight(&mut self, weight: N64) -> &mut Self {
        #[cfg(feature = "multiweight")]
        self.weights.push(weight);
        #[cfg(not(feature = "multiweight"))]
        {
            self.weights = weight;
        }
        self
    }

    /// Rescale all energies and momenta
    pub fn rescale_energies(&mut self, scale: N64) {
        for (_, p) in &mut self.outgoing_by_pid {
            *p =
                [scale * p[0], scale * p[1], scale * p[2], scale * p[3]].into();
        }
    }

    /// Construct an event
    pub fn build(self) -> Event {
        let outgoing_by_pid = compress_outgoing(self.outgoing_by_pid);
        Event {
            id: Default::default(),
            #[cfg(feature = "multiweight")]
            weights: RwLock::new(Weights(self.weights.into_boxed_slice())),
            #[cfg(not(feature = "multiweight"))]
            weights: RwLock::new(Weights(self.weights)),
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
        outgoing_by_pid
            .into_iter()
            .map(|(id, p)| (id, p.into_boxed_slice())),
    );
    outgoing_by_pid.into_boxed_slice()
}

/// A Monte Carlo scattering event
#[derive(Deserialize, Serialize, Debug, Default, Derivative)]
#[derivative(PartialEq, Eq, Ord)]
pub struct Event {
    /// Event id
    pub id: usize,
    #[derivative(PartialEq = "ignore")]
    #[derivative(Ord = "ignore")]
    /// Event weights
    pub weights: RwLock<Weights>,

    outgoing_by_pid: Box<[(ParticleID, MomentumSet)]>,
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

const EMPTY_SLICE: &[FourVector] = &[];

impl Event {
    /// Construct an empty event
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
        self.weights.read().central()
    }

    /// Extract the outgoing particle momenta grouped by particle id
    pub fn into_outgoing(self) -> Box<[(ParticleID, MomentumSet)]> {
        self.outgoing_by_pid
    }

    /// Number of weights
    pub fn n_weights(&self) -> usize {
        self.weights.read().len()
    }

    /// Rescale weights by some factor
    pub fn rescale_weights(&self, scale: N64) {
        for wt in self.weights.write().iter_mut() {
            *wt *= scale
        }
    }
}

/// Event weights
#[cfg(feature = "multiweight")]
#[derive(
    Deserialize,
    Serialize,
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
pub struct Weights(Box<[N64]>);
/// Event weights
#[cfg(not(feature = "multiweight"))]
#[derive(
    Deserialize,
    Serialize,
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
pub struct Weights(N64);

impl Weights {
    /// Create central weight
    pub fn new_single(w: N64) -> Self {
        #[cfg(feature = "multiweight")]
        return Self(vec![w].into_boxed_slice());

        #[cfg(not(feature = "multiweight"))]
        Self(w)
    }

    /// Get central weight
    pub fn central(&self) -> N64 {
        #[cfg(feature = "multiweight")]
        return self.0[0];

        #[cfg(not(feature = "multiweight"))]
        self.0
    }

    /// Get number of weights
    pub fn len(&self) -> usize {
        #[cfg(feature = "multiweight")]
        return self.0.len();

        #[cfg(not(feature = "multiweight"))]
        1
    }

    /// Check if there are any weights
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterator over weights
    pub fn iter(&self) -> impl Iterator<Item = &N64> {
        #[cfg(feature = "multiweight")]
        return self.0.iter();

        #[cfg(not(feature = "multiweight"))]
        std::iter::once(&self.0)
    }

    /// Mutating iterator over weights
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut N64> {
        #[cfg(feature = "multiweight")]
        return self.0.iter_mut();

        #[cfg(not(feature = "multiweight"))]
        std::iter::once(&mut self.0)
    }

    /// Copy weight entries
    pub fn copy_from(&mut self, rhs: &Weights) {
        #[cfg(feature = "multiweight")]
        return self.0.copy_from_slice(&rhs.0);

        #[cfg(not(feature = "multiweight"))]
        {
            self.0 = rhs.0;
        }
    }
}

impl AddAssign<&Weights> for Weights {
    fn add_assign(&mut self, rhs: &Weights) {
        use itertools::zip_eq;
        for (lhs, rhs) in zip_eq(self.iter_mut(), rhs.iter()) {
            *lhs += rhs;
        }
    }
}
