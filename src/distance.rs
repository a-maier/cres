use crate::event::Event;
use crate::four_vector::FourVector;

use std::cmp::Ordering;

use noisy_float::prelude::*;
use permutohedron::LexicalPermutation;

/// A metric (distance function) in the space of all events
pub trait Distance {
    fn distance(&self, ev1: &Event, ev2: &Event) -> N64;
}

const FALLBACK_SIZE: usize = 8;

/// The distance function defined in [arXiv:2109.07851](https://arxiv.org/abs/2109.07851)
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct EuclWithScaledPt {
    pt_weight: N64,
}

impl Distance for EuclWithScaledPt {
    fn distance(&self, ev1: &Event, ev2: &Event) -> N64 {
        let mut dist = n64(0.);
        let out1 = ev1.outgoing();
        let out2 = ev2.outgoing();
        let mut idx1 = 0;
        let mut idx2 = 0;
        while idx1 < out1.len() && idx2 < out2.len() {
            let (t1, p1) = &out1[idx1];
            let (t2, p2) = &out2[idx2];
            match t1.cmp(t2) {
                Ordering::Greater => {
                    dist += self.pt_norm(p1);
                    idx1 += 1;
                }
                Ordering::Less => {
                    dist += self.pt_norm(p2);
                    idx2 += 1;
                }
                Ordering::Equal => {
                    dist += self.set_distance(p1, p2);
                    idx1 += 1;
                    idx2 += 1;
                }
            }
        }

        // consume remainders
        debug_assert!(idx1 >= out1.len() || idx2 >= out2.len());
        if idx1 < out1.len() {
            dist += out1[idx1..]
                .iter()
                .map(|(_t, p)| self.pt_norm(p))
                .sum::<N64>();
        } else if idx2 < out2.len() {
            dist += out2[idx2..]
                .iter()
                .map(|(_t, p)| self.pt_norm(p))
                .sum::<N64>();
        }
        dist
    }
}

impl EuclWithScaledPt {
    /// Distance function with the given parameter τ = `pt_weight`
    ///
    /// See [arXiv:2109.07851](https://arxiv.org/abs/2109.07851) for a
    /// definition of τ
    pub fn new(pt_weight: N64) -> Self {
        EuclWithScaledPt { pt_weight }
    }

    fn pt_norm(&self, p: &[FourVector]) -> N64 {
        p.iter().map(|p| pt_norm(p, self.pt_weight)).sum()
    }

    fn set_distance(&self, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        if std::cmp::max(p1.len(), p2.len()) < FALLBACK_SIZE {
            self.min_paired_distance(p1, p2)
        } else {
            self.norm_ordered_paired_distance(p1, p2)
        }
    }

    fn min_paired_distance(&self, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        if p1.len() > p2.len() {
            return self.min_paired_distance(p2, p1);
        }
        debug_assert!(p1.len() <= p2.len());
        // copy and pad with zeros
        let zero = FourVector::new();
        let mut p1: Vec<_> = p1.iter().copied().collect();
        p1.resize_with(p2.len(), || zero);
        p1.sort_unstable();
        let mut min_dist = self.paired_distance(&p1, p2);
        while p1.next_permutation() {
            min_dist = std::cmp::min(min_dist, self.paired_distance(&p1, p2));
        }
        min_dist
    }

    fn paired_distance(&self, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        debug_assert!(p1.len() == p2.len());
        p1.iter()
            .zip(p2.iter())
            .map(|(p1, p2)| pt_dist(p1, p2, self.pt_weight))
            .sum()
    }

    fn norm_ordered_paired_distance(
        &self,
        p1: &[FourVector],
        p2: &[FourVector],
    ) -> N64 {
        if p1.len() > p2.len() {
            return self.norm_ordered_paired_distance(p2, p1);
        }
        let mut p1: Vec<_> = p1.iter().copied().collect();
        p1.resize_with(p2.len(), FourVector::new);
        std::cmp::min(
            self.ordered_paired_distance_eq_size(&p1, p2),
            self.ordered_paired_distance_eq_size(p2, &p1),
        )
    }

    fn ordered_paired_distance_eq_size(
        &self,
        p1: &[FourVector],
        p2: &[FourVector],
    ) -> N64 {
        debug_assert!(p1.len() == p2.len());
        let mut dists: Vec<_> = p2.iter().map(|q| (n64(0.), q)).collect();
        let mut dist = n64(0.);
        for p in p1 {
            for (dist, q) in &mut dists {
                *dist = pt_dist_sq(p, *q, self.pt_weight);
            }
            let (n, min) =
                dists.iter().enumerate().min_by_key(|(_n, d)| *d).unwrap();
            dist += min.0.sqrt();
            dists.swap_remove(n);
        }
        dist
    }
}

pub fn pt_norm(p: &FourVector, pt_weight: N64) -> N64 {
    pt_norm_sq(p, pt_weight).sqrt()
}

pub fn pt_norm_sq(p: &FourVector, pt_weight: N64) -> N64 {
    let pt = pt_weight * p.pt();
    p.spatial_norm_sq() + pt * pt
}

fn pt_dist(p: &FourVector, q: &FourVector, pt_weight: N64) -> N64 {
    pt_dist_sq(p, q, pt_weight).sqrt()
}

fn pt_dist_sq(p: &FourVector, q: &FourVector, pt_weight: N64) -> N64 {
    let dpt = pt_weight * (p.pt() - q.pt());
    (*p - *q).spatial_norm_sq() + dpt * dpt
}
