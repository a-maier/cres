use std::{cmp::PartialOrd, sync::Mutex};

use log::debug;
use rayon::prelude::*;

use crate::progress_bar::{ProgressBar, Progress};

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
    circle_partition_with_callback(s, dist, depth, |_|{})
}

pub fn circle_partition_with_progress<DF, D, T>(
    s: &mut [T],
    dist: DF,
    depth: u32,
) -> Vec<&mut [T]>
where
    DF: Send + Sync + Fn(&T, &T) -> D,
    D: PartialOrd,
    T: Send + Sync
{
    if depth > 0 {
        let max_progress = depth as u64 * 2_u64.pow(depth - 1);
        let progress = ProgressBar::new(max_progress, "");
        let inc = |d| progress.inc(2_u64.pow(d - 1));
        let res = circle_partition_with_callback(s, dist, depth, inc);
        progress.finish();
        res
    } else {
        circle_partition(s, dist, depth)
    }
}

pub fn circle_partition_with_callback<C, DF, D, T>(
    s: &mut [T],
    dist: DF,
    depth: u32,
    callback: C,
) -> Vec<&mut [T]>
where
    DF: Send + Sync + Fn(&T, &T) -> D,
    D: PartialOrd,
    T: Send + Sync,
    C: Copy + Send + Sync + Fn(u32),
{
    if depth == 0 {
        return vec![s]
    }
    if let Some(corner) = find_corner(s, &dist) {
        debug_assert!(!s.is_empty());
        let res = Mutex::new(Vec::new());
        let last_idx = s.len() - 1;
        s.swap(corner, last_idx);
        partition(s, &dist, depth, &res, callback);
        res.into_inner().unwrap()
    } else {
        debug_assert!(s.is_empty());
        vec![s]
    }
}


pub(crate) fn find_corner<D, DF, P>(
    slice: &[P],
    dist: &DF
) -> Option<usize>
where
    P: Send + Sync,
    DF: Send + Sync + Fn(&P, &P) -> D,
    D: PartialOrd,
{
    if let Some((first, rest)) = slice.split_first() {
        debug!("Finding corner");
        let max = rest.par_iter().enumerate().max_by(
            |(_, a), (_, b)| dist(first, *a).partial_cmp(&dist(first, *b)).unwrap()
        );
        if let Some((pos, _)) = max {
            debug!("Corner at {pos}");
            Some(pos + 1)
        } else {
            debug!("Corner at 0");
            Some(0)
        }
    } else {
        None
    }
}

fn partition<'a, 'b, 'c, C, D, DF, T>(
    s: &'a mut[T],
    dist: &'b DF,
    depth: u32,
    res: &'c Mutex<Vec<&'a mut[T]>>,
    callback: C,
)
where
    T: Send + Sync,
    DF: Send + Sync,
    for<'d, 'e> DF: Fn(&'d T, &'e T) -> D,
    D: PartialOrd,
    C: Copy + Send + Sync + Fn(u32),
{
    if depth == 0 || s.len() < 2 {
        res.lock().unwrap().push(s);
        return;
    }
    debug!("Starting partition at depth {depth}");
    s.swap(0, s.len() - 1);
    let (centre, rest) = s.split_first_mut().unwrap();
    rest.par_sort_unstable_by(
        |a, b| dist(centre, a).partial_cmp(&dist(centre, b)).unwrap()
    );
    let median_idx = s.len() / 2;
    let (inner, outer) = s.split_at_mut(median_idx);
    callback(depth);
    debug!("Finished partition at depth {depth}");
    [inner, outer].into_par_iter().for_each(
        |region| partition(region, dist, depth - 1, res, callback)
    );
}
