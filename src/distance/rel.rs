use std::{collections::HashMap, cmp::Ordering};

use jetty::PseudoJet;
use noisy_float::prelude::*;
use particle_id::{ParticleID, sm_elementary_particles::photon};
use permutohedron::LexicalPermutation;

use crate::{event::Event, four_vector::FourVector};

use super::Distance;

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
        let mut dist = n64(0.);
        let mut out1 = ev1.outgoing().iter().peekable();
        let mut out2 = ev2.outgoing().iter().peekable();
        loop {
            match (out1.peek(), out2.peek()) {
                (None, None) => break,
                (Some(_), None) | (None, Some(_)) => {
                    todo!("Relative distance for mismatched particle types");
                },
                (Some((t1, p1)), Some((t2, p2))) => match t1.cmp(t2) {
                    Ordering::Less | Ordering::Greater => {
                        todo!("Relative distance for mismatched particle types");
                    },
                    Ordering::Equal => {
                        let d = self.set_distance(*t1, p1, p2);
                        dist = std::cmp::max(d, dist);
                        out1.next();
                        out2.next();
                    }
                }
            }
        }
        dist
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
    if p1.len() != p2.len() {
        todo!("Relative distance for mismatched number of particles");
    }
    let mut p1 = p1.to_vec();

    let mut min_dist = paired_distance(p_scale, delta_r_scale, &p1, p2);
    while p1.next_permutation() {
        min_dist = std::cmp::min(min_dist, paired_distance(p_scale, delta_r_scale, &p1, p2));
    }
    min_dist
}

fn paired_distance(p_scale: N64, delta_r_scale: N64, p1: &[FourVector], p2: &[FourVector]) -> N64 {
    p1.iter().zip(p2)
        .map(|(p1, p2)| momentum_distance(p_scale, delta_r_scale, *p1, *p2))
        .max()
        .unwrap_or(n64(0.))
}

fn momentum_distance(p_scale: N64, delta_r_scale: N64, p1: FourVector, p2: FourVector) -> N64 {
    let delta_p_sq = (p1 - p2).spatial_norm_sq();
    let min_p_sq = std::cmp::min(p1.spatial_norm_sq(), p2.spatial_norm_sq());
    let rel_p_diff = (delta_p_sq / min_p_sq).sqrt();
    let delta_r = PseudoJet::from(p1).delta_r(&p2.into());
    std::cmp::max(p_scale * rel_p_diff, delta_r_scale * delta_r)
}
