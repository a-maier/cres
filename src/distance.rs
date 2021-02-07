use crate::event::Event;
use crate::four_vector::FourVector;

use std::cmp::Ordering;

use noisy_float::prelude::*;
use permutohedron::LexicalPermutation;

pub fn distance(ev1: &Event, ev2: &Event) -> N64 {
    let mut it1 = ev1.outgoing_by_pid.iter();
    let mut it2 = ev2.outgoing_by_pid.iter();
    let mut n1 = it1.next();
    let mut n2 = it2.next();
    let mut dist = n64(0.);
    while let (Some((t1, p1)), Some((t2, p2))) = (n1, n2) {
        match t1.cmp(t2) {
            Ordering::Less => {
                dist += euclid_norm(p1);
                n1 = it1.next();
            }
            Ordering::Greater => {
                dist += euclid_norm(p2);
                n2 = it2.next();
            }
            Ordering::Equal => {
                dist += min_paired_distance(p1, p2);
                n1 = it1.next();
                n2 = it2.next();
            }
        }
    }

    // consume remainders
    dist += it1.map(|(_t, p)| euclid_norm(p)).sum::<N64>();
    dist += it2.map(|(_t, p)| euclid_norm(p)).sum::<N64>();
    dist
}

fn euclid_norm(p: &[FourVector]) -> N64 {
    p.iter().map(|p| p.euclid_norm()).sum()
}

fn min_paired_distance(p1: &[FourVector], p2: &[FourVector]) -> N64 {
    if p1.len() > p2.len() {
        return min_paired_distance(p2, p1);
    }
    debug_assert!(p1.len() <= p2.len());
    // copy and pad with zeros
    let zero = FourVector::new();
    let mut p1: Vec<_> = p1.iter().copied().collect();
    p1.resize_with(p2.len(), || zero);
    p1.sort_unstable();
    let mut min_dist = paired_distance(&p1, &p2);
    while p1.next_permutation() {
        min_dist = std::cmp::min(min_dist, paired_distance(&p1, &p2));
    }
    min_dist
}

fn paired_distance(p1: &[FourVector], p2: &[FourVector]) -> N64 {
    //debug_assert!(p1.len() == p2.len());
    p1.iter()
        .zip(p2.iter())
        .map(|(p1, p2)| (*p1 - *p2).euclid_norm())
        .sum()
}
