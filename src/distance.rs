use crate::event::Event;
use crate::four_vector::FourVector;

use std::cmp::Ordering;

use noisy_float::prelude::*;
use permutohedron::LexicalPermutation;

const FALLBACK_SIZE: usize = 8;

pub fn distance(ev1: &Event, ev2: &Event) -> N64 {
    let mut dist = n64(0.);
    let out1 = &ev1.outgoing_by_pid;
    let out2 = &ev2.outgoing_by_pid;
    let mut idx1 = 0;
    let mut idx2 = 0;
    while idx1 < out1.len() && idx2 < out2.len() {
        let (t1, p1) = &out1[idx1];
        let (t2, p2) = &out2[idx2];
        match t1.cmp(&t2) {
            Ordering::Less => {
                dist += euclid_norm(&p1);
                idx1 += 1;
            },
            Ordering::Greater => {
                dist += euclid_norm(&p2);
                idx2 += 1;
            },
            Ordering::Equal => {
                dist += set_distance(&p1, &p2);
                idx1 += 1;
                idx2 += 1;
            }
        }
    }

    // consume remainders
    debug_assert!(idx1 >= out1.len() || idx2 >= out2.len());
    if idx1 < out1.len() {
        dist += out1[idx1..].iter().map(|(_t, p)| euclid_norm(p)).sum::<N64>();
    } else if idx2 < out2.len() {
        dist += out2[idx2..].iter().map(|(_t, p)| euclid_norm(p)).sum::<N64>();
    }
    dist
}

fn euclid_norm(p: &[FourVector]) -> N64 {
    p.iter().map(|p| p.euclid_norm()).sum()
}

fn set_distance(p1: &[FourVector], p2: &[FourVector]) -> N64 {
    if std::cmp::max(p1.len(), p2.len()) < FALLBACK_SIZE {
        min_paired_distance(p1, p2)
    } else {
        norm_ordered_paired_distance(p1, p2)
    }
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
    debug_assert!(p1.len() == p2.len());
    p1.iter()
        .zip(p2.iter())
        .map(|(p1, p2)| (*p1 - *p2).euclid_norm())
        .sum()
}

fn norm_ordered_paired_distance(p1: &[FourVector], p2: &[FourVector]) -> N64 {
    if p1.len() > p2.len() {
        return norm_ordered_paired_distance(p2, p1);
    }
    let mut p1: Vec<_> = p1.iter().copied().collect();
    p1.resize_with(p2.len(), FourVector::new);
    std::cmp::min(
        ordered_paired_distance_eq_size(&p1, p2),
        ordered_paired_distance_eq_size(p2, &p1)
    )
}

fn ordered_paired_distance_eq_size(p1: &[FourVector], p2: &[FourVector]) -> N64 {
    debug_assert!(p1.len() == p2.len());
    let mut dists: Vec<_> = p2.iter().map(|q| (n64(0.), q)).collect();
    let mut dist = n64(0.);
    for p in p1 {
        for (dist, q) in &mut dists {
            *dist = (*p - **q).euclid_norm_sq();
        }
        let (n, min) = dists.iter().enumerate()
            .min_by_key(|(_n, d)| *d).unwrap();
        dist += min.0.sqrt();
        dists.swap_remove(n);
    }
    dist
}
