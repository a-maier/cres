use std::{cmp::Ord, sync::Mutex, ops::Range};

use log::debug;
use num_traits::Zero;
use rayon::prelude::*;

use crate::progress_bar::{ProgressBar, Progress};

pub fn circle_partition<DF, D, T>(
    s: &mut Vec<T>,
    dist: DF,
    depth: u32,
) -> Vec<&mut [T]>
where
    DF: Send + Sync + Fn(&T, &T) -> D,
    D: Copy + Ord + Zero + Send + Sync,
    T: Send + Sync
{
    circle_partition_with_callback(s, dist, depth, |_|{})
}

pub fn circle_partition_with_progress<DF, D, T>(
    s: &mut Vec<T>,
    dist: DF,
    depth: u32,
) -> Vec<&mut [T]>
where
    DF: Send + Sync + Fn(&T, &T) -> D,
    D: Copy + Ord + Zero + Send + Sync,
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
    s: &mut Vec<T>,
    dist: DF,
    depth: u32,
    callback: C,
) -> Vec<&mut [T]>
where
    DF: Send + Sync + Fn(&T, &T) -> D,
    D: Copy + Ord + Zero + Send + Sync,
    T: Send + Sync,
    C: Copy + Send + Sync + Fn(u32),
{
    if depth == 0 {
        return vec![s]
    }
    // optimisation: we replace the points to partition by pairs
    // (distance, point), where the `distance` entry is used to
    // cache results of distance computations
    let mut pts = Vec::from_iter(
        s.drain(..).map(|t| (D::zero(), t))
    );
    if let Some(corner) = find_corner(&mut pts, &dist) {
        debug_assert!(!pts.is_empty());
        // instead of the slices representing partitions
        // we first get ranges of indices in vector of points
        // this makes it evident to the borrow checker that nothing
        // untoward is going on
        let ranges = Mutex::new(Vec::new());
        let last_idx = pts.len() - 1;
        pts.swap(corner, last_idx);
        let len = pts.len();
        partition(&mut pts, 0..len, &dist, depth, &ranges, callback);
        s.extend(pts.into_iter().map(|(_d, p)| p));
        ranges_to_slices(ranges.into_inner().unwrap(), s)
    } else {
        debug_assert!(s.is_empty());
        vec![s]
    }
}

fn ranges_to_slices<P>(
    mut ranges: Vec<Range<usize>>,
    mut rest: &mut[P]
) -> Vec<&mut [P]> {
    let mut res = Vec::with_capacity(ranges.len());
    ranges.sort_by_key(|r| r.start);
    ranges.pop();
    for range in ranges {
        let (next, r) = rest.split_at_mut(range.end - range.start);
        res.push(next);
        rest = r;
    }
    res.push(rest);
    res
}

pub(crate) fn find_corner<D, DF, P>(
    slice: &mut [(D, P)],
    dist: &DF
) -> Option<usize>
where
    P: Send + Sync,
    DF: Send + Sync + Fn(&P, &P) -> D,
    D: Copy + Ord + Send + Sync,
{
    if let Some((first, rest)) = slice.split_first_mut() {
        debug!("Finding corner");
        rest.par_iter_mut().for_each(
            |(d, p)| *d = dist(&first.1, p)
        );
        let max = rest.par_iter().enumerate().max_by_key(
            |(_, (d, _))| *d
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

fn partition<C, D, DF, T>(
    s: &mut[(D, T)],
    cur_range: Range<usize>,
    dist: &DF,
    depth: u32,
    res: &Mutex<Vec<Range<usize>>>,
    callback: C,
)
where
    T: Send + Sync,
    DF: Send + Sync,
    for<'a, 'b> DF: Fn(&'a T, &'b T) -> D,
    D: Copy + Ord + Send + Sync,
    C: Copy + Send + Sync + Fn(u32),
{
    debug_assert_eq!((cur_range.end - cur_range.start) as usize, s.len());
    if depth == 0 || s.len() < 2 {
        res.lock().unwrap().push(cur_range);
        return;
    }
    debug!("Starting partition at depth {depth}");
    s.swap(0, s.len() - 1);
    let (centre, rest) = s.split_first_mut().unwrap();
    rest.par_iter_mut().for_each(
        |(d, p)| *d = dist(&centre.1, p)
    );
    rest.par_sort_unstable_by_key(|(d, _)| *d);
    let median_idx = s.len() / 2;
    let (inner, outer) = s.split_at_mut(median_idx);
    let mid = cur_range.start + median_idx;
    let inner_range = cur_range.start..mid;
    let outer_range = mid..cur_range.end;
    callback(depth);
    debug!("Finished partition at depth {depth}");
    [(inner, inner_range), (outer, outer_range)].into_par_iter().for_each(
        |(region, range)| partition(region, range, dist, depth - 1, res, callback)
    );
}
