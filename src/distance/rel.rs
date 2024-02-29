use std::collections::HashMap;

use jetty::PseudoJet;
use noisy_float::prelude::*;
use particle_id::{ParticleID, sm_elementary_particles::photon};
use permutohedron::LexicalPermutation;

use crate::{event::Event, four_vector::FourVector};

use super::{Distance, EuclWithScaledPt};

/// Distance based on the maximum relative spatial momentum differences and ΔR
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaxRelWithDeltaR {
    /// Particle-dependent scale factors for relative momentum differences
    ///
    /// For example, a scale factor of 2 means that a relative
    /// momentum difference of 10% contributes 0.2 to the distance. If
    /// no explicit scale factor is given the
    /// [DEFAULT_MOMENTUM_SCALE](Self::DEFAULT_MOMENTUM_SCALE)
    /// will be used.
    pub momentum_scale: HashMap<ParticleID, N64>,

    /// Particle-dependent scale factors for ΔR
    ///
    /// For example, a scale factor of 2 means that a ΔR of 0.1
    /// contributes 0.2 to the distance. If
    /// no explicit scale factor is given the
    /// [DEFAULT_DELTA_R_SCALE](Self::DEFAULT_DELTA_R_SCALE)
    /// will be used.
    pub delta_r_scale: HashMap<ParticleID, N64>,
}

impl Default for MaxRelWithDeltaR {
    fn default() -> Self {
        // TODO: tweak
        Self {
            momentum_scale: [
                (photon, n64(10.))
            ].into(),
            delta_r_scale: Default::default()
        }
    }
}

impl Distance for MaxRelWithDeltaR {
    fn distance(&self, ev1: &Event, ev2: &Event) -> N64 {
        if same_particle_types_and_multiplicities(ev1, ev2) {
            ev1.outgoing().iter()
                .zip(ev2.outgoing())
                .map(|((t1, p1), (t2, p2))| {
                    debug_assert_eq!(t1, t2);
                    self.set_distance(*t1, p1, p2)
                })
                .max()
                .unwrap_or(n64(0.))
        } else {
            EuclWithScaledPt::new(n64(0.)).distance(ev1, ev2)
        }
    }
}

impl MaxRelWithDeltaR {
    /// Default scale factor for relative momentum differences
    ///
    /// For example, a scale factor of 2 means that a relative
    /// momentum difference of 10% contributes 0.2 to the distance.
    pub const DEFAULT_MOMENTUM_SCALE: f64 = 2.;

    /// Default scale factor for ΔR
    ///
    /// For example, a scale factor of 1 means that a ΔR of 0.1
    /// contributes 0.1 to the distance.
    pub const DEFAULT_DELTA_R_SCALE: f64 = 1.;

    fn set_distance(&self, t: ParticleID, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        debug_assert_eq!(p1.len(), p2.len());
        let p_scale = self.momentum_scale.get(&t)
            .copied()
            .unwrap_or(n64(Self::DEFAULT_MOMENTUM_SCALE));
        let delta_r_scale = self.delta_r_scale.get(&t)
            .copied()
            .unwrap_or(n64(Self::DEFAULT_DELTA_R_SCALE));
        min_paired_distance(p_scale, delta_r_scale, p1, p2)
    }

}

fn min_paired_distance(p_scale: N64, delta_r_scale: N64, p1: &[FourVector], p2: &[FourVector]) -> N64 {
    debug_assert_eq!(p1.len(), p2.len());
    let mut p1 = p1.to_vec();
    let mut min_dist = paired_distance(p_scale, delta_r_scale, &p1, p2);
    while p1.next_permutation() {
        min_dist = std::cmp::min(min_dist, paired_distance(p_scale, delta_r_scale, &p1, p2));
    }
    min_dist
}

fn paired_distance(p_scale: N64, delta_r_scale: N64, p1: &[FourVector], p2: &[FourVector]) -> N64 {
    debug_assert_eq!(p1.len(), p2.len());
    p1.iter().zip(p2)
        .map(|(p1, p2)| momentum_distance(p_scale, delta_r_scale, *p1, *p2))
        .max()
        .unwrap_or(n64(0.))
}

fn momentum_distance(p_scale: N64, delta_r_scale: N64, p1: FourVector, p2: FourVector) -> N64 {
    let rel_p_diff = (p1.spatial_norm_sq() / p2.spatial_norm_sq()).ln().abs();
    let delta_r = PseudoJet::from(p1).delta_r(&p2.into());
    std::cmp::max(p_scale * rel_p_diff, delta_r_scale * delta_r)
}

fn same_particle_types_and_multiplicities(ev1: &Event, ev2: &Event) -> bool {
    ev1.outgoing().len() == ev2.outgoing().len()
        && ev1.outgoing().iter()
        .zip(ev2.outgoing())
        .all(|((t1, out1), (t2, out2))| t1 == t2 && out1.len() == out2.len())
}
