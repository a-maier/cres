use std::cmp::PartialOrd;

use rayon::prelude::*;

pub fn circle_partition<DF, D, T>(
    s: &mut [T],
    dist: DF,
    depth: u32,
) -> Vec<&mut [T]>
where
    DF: Send + Sync + Fn(&T, &T) -> D,
    D: PartialOrd,
    T: Send + Sync
{
    if depth == 0 {
        return vec![s]
    }
    if let Some(corner) = find_corner(s.iter(), &dist) {
        debug_assert!(!s.is_empty());
        let mut res = Vec::new();
        let last_idx = s.len() - 1;
        s.swap(corner, last_idx);
        partition(s, &dist, depth, &mut res);
        res
    } else {
        debug_assert!(s.is_empty());
        vec![s]
    }
}


pub(crate) fn find_corner<D, DF, I, P>(
    iter: I,
    dist: &DF
) -> Option<usize>
where
    I: IntoIterator<Item = P>,
    DF: Send + Sync + Fn(P, P) -> D,
    P: Copy,
    D: PartialOrd,
{
    let mut iter = iter.into_iter();
    if let Some(first) = iter.next() {
        let max = iter.enumerate().max_by(
            |(_, a), (_, b)| dist(first, *a).partial_cmp(&dist(first, *b)).unwrap()
        );
        if let Some((pos, _)) = max {
            Some(pos + 1)
        } else {
            Some(0)
        }
    } else {
        None
    }
}

fn partition<'a, 'b, 'c, D, DF, T>(
    s: &'a mut[T],
    dist: &'b DF,
    depth: u32,
    res: &'c mut Vec<&'a mut[T]>,
)
where
    T: Send + Sync,
    DF: Send + Sync,
    for<'d, 'e> DF: Fn(&'d T, &'e T) -> D,
    D: PartialOrd,
{
    if depth == 0 || s.len() < 2 {
        res.push(s);
        return;
    }
    s.swap(0, s.len() - 1);
    let (centre, rest) = s.split_first_mut().unwrap();
    rest.par_sort_unstable_by(
        |a, b| dist(centre, a).partial_cmp(&dist(centre, b)).unwrap()
    );
    let median_idx = s.len() / 2;
    let (inner, outer) = s.split_at_mut(median_idx);
    partition(inner, dist, depth - 1, res);
    partition(outer, dist, depth - 1, res);
}
