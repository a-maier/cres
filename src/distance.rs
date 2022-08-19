use crate::event::Event;
use crate::four_vector::FourVector;

use std::cmp::Ordering;
use std::fmt::{Display, self};
use std::ops::{Index, IndexMut};

use itertools::Itertools;
use noisy_float::prelude::*;
use pathfinding::prelude::{Weights, kuhn_munkres_min};
use permutohedron::LexicalPermutation;

/// A metric (distance function) in the space of all events
pub trait Distance<E=Event> {
    fn distance(&self, ev1: &E, ev2: &E) -> N64;
}

impl<D, E> Distance<E> for &D where D: Distance<E> {
    fn distance(&self, ev1: &E, ev2: &E) -> N64 {
        (*self).distance(ev1, ev2)
    }
}

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
        self.min_paired_distance(p1, p2)
    }

    fn min_paired_distance(&self, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        if p1.len() > p2.len() {
            return self.min_paired_distance(p2, p1);
        }
        debug_assert!(p1.len() <= p2.len());
        // copy and pad with zeros
        let zero = FourVector::new();
        let mut p1 = p1.to_vec();
        p1.resize_with(p2.len(), || zero);
        p1.sort_unstable();

        // TODO: find optimum value (either 3 or 4)
        const MAX_PART_NAIVE: usize = 3;
        match p1.len() {
            0 => n64(0.),
            1 => pt_dist(&p1[0], &p2[0], self.pt_weight),
            2..=MAX_PART_NAIVE => self.min_paired_distance_naive(&mut p1, p2),
            _ => self.min_paired_distance_hungarian(&p1, p2),
        }
    }

    fn min_paired_distance_naive(&self, p1: &mut [FourVector], p2: &[FourVector]) -> N64 {
        let mut min_dist = self.paired_distance(p1, p2);
        while p1.next_permutation() {
            min_dist = std::cmp::min(min_dist, self.paired_distance(p1, p2));
        }
        min_dist
    }

    fn min_paired_distance_hungarian(&self, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        let weights = SquareMatrix::from_iter(
            p1.iter().cartesian_product(p2.iter())
                .map(|(p, q)| pt_dist(p, q, self.pt_weight))
        );
        kuhn_munkres_min(&weights).0
    }

    fn paired_distance(&self, p1: &[FourVector], p2: &[FourVector]) -> N64 {
        debug_assert!(p1.len() == p2.len());
        p1.iter()
            .zip(p2.iter())
            .map(|(p1, p2)| pt_dist(p1, p2, self.pt_weight))
            .sum()
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

pub struct PtDistance<'a, 'b, D: Distance> {
    ev_dist: &'a D,
    events: &'b [Event],
}

impl<'a, 'b, D: Distance>  PtDistance<'a, 'b, D> {
    pub fn new(ev_dist: &'a D, events: &'b [Event]) -> Self {
        Self { ev_dist, events }
    }
}

impl<'a, 'b, D: Distance> Distance<usize> for PtDistance<'a, 'b, D> {
    fn distance(&self, e1: &usize, e2: &usize) -> N64 {
        self.ev_dist.distance(&self.events[*e1], &self.events[*e2])
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct SquareMatrix {
    entries: Vec<N64>,
    rows: usize,
}

impl Index<(usize, usize)> for SquareMatrix {
    type Output = N64;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (row, col) = index;
        &self.entries[row * self.rows + col]
    }
}

impl IndexMut<(usize, usize)> for SquareMatrix {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (row, col) = index;
        &mut self.entries[row * self.rows + col]
    }
}

impl Weights<N64> for SquareMatrix {
    fn rows(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.rows()
    }

    fn at(&self, row: usize, col: usize) -> N64 {
        self[(row, col)]
    }

    fn neg(&self) -> Self {
        let entries = self.entries.iter().map(|e| -e).collect();
        Self { entries, rows: self.rows }
    }
}

impl Display for SquareMatrix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for row in 0..self.rows {
            write!(f, "(")?;
            for col in 0..self.rows {
                write!(f, " {:^6.2} ", self[(row, col)])?;
            }
            writeln!(f, ")")?;
        }
        Ok(())
    }
}
impl FromIterator<N64> for SquareMatrix {
    fn from_iter<T: IntoIterator<Item = N64>>(iter: T) -> Self {
        let entries = Vec::from_iter(iter);
        let rows = (entries.len() as f64).sqrt();
        assert_eq!(rows.fract(), 0.);
        Self { entries, rows: rows as usize }
    }
}
