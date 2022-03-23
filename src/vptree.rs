use std::cmp::PartialOrd;
use std::default::Default;
use std::iter::FromIterator;
use std::ops::Sub;

use log::{debug, trace};
use num_traits::sign::Signed;

use crate::sorted_multimap::SortedMultimap;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct VPTree<P, D> {
    nodes: Vec<Node<P, D>>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Node<P, D> {
    vantage_pt: P,
    children: Option<Children<D>>
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Children<D> {
    radius: D,
    outside_offset: usize,
}

pub trait Dist<P = Self> {
    type Output: Copy + Default + PartialOrd + Signed + Sub;

    fn dist(&self, p: &P) -> Self::Output;
}

impl<P: Dist> VPTree<P, <P as Dist>::Output> {
    pub fn new(nodes: Vec<P>) -> Self {
        Self::from_iter(nodes.into_iter())
    }

    pub fn nearest(&self, pt: &P) -> Option<(&P, <P as Dist>::Output)> {
        self.nearest_in(pt, |p, q| p.dist(q))
    }

    pub fn k_nearest(
        &self,
        pt: &P,
        k: usize
    ) -> SortedMultimap<<P as Dist>::Output, &P> {
        self.k_nearest_in(pt, k, |p, q| p.dist(q))
    }

    // pub fn fill_to_k_nearest<'a, 'b: 'a>(
    //     &'b self,
    //     pt: &P,
    //     k: usize,
    //     candidates: &mut SortedMultimap<<P as Dist>::Output, &'a P>
    // ) {
    //     self.fill_to_k_nearest_in(pt, k, candidates, |p, q| p.dist(q))
    // }
}

impl<P: Dist> FromIterator<P> for VPTree<P, <P as Dist>::Output> {
    fn from_iter<I: IntoIterator<Item=P>>(iter: I) -> Self {
        Self::from_iter_with_dist(iter, |p, q| p.dist(q))
    }
}

impl<P, D: Copy + Default + PartialOrd + Signed + Sub> VPTree<P, D> {
    pub fn new_with_dist<DF>(
        nodes: Vec<P>,
        dist: DF
    ) -> Self
    where
        DF: Copy,
        for<'a, 'b> DF: Fn(&'a P, &'b P) -> D
    {
        Self::from_iter_with_dist(nodes.into_iter(), dist)
    }
}

impl<'x, P: 'x, D: Copy + Default + PartialOrd + Signed + Sub> VPTree<P, D> {
    pub fn from_iter_with_dist<DF, I>(
        iter: I,
        dist: DF
    ) -> Self
    where
        I: IntoIterator<Item = P>,
        DF: Copy,
        for<'a, 'b> DF: Fn(&'a P, &'b P) -> D
    {
        let mut nodes = Vec::from_iter(
            iter.into_iter().map(
                |vantage_pt| {
                    // reserve first element for storing distances
                    (Default::default(), Node{ vantage_pt, children: None })
                }
            )
        );
        let corner_pt_idx = Self::find_corner_pt(
            nodes.iter().map(|(_, pt)| &pt.vantage_pt),
            dist
        );
        debug!("first vantage point: {corner_pt_idx:?}");
        if let Some(pos) = corner_pt_idx {
            let last_idx = nodes.len() - 1;
            nodes.swap(pos, last_idx)
        }
        Self::build_tree(nodes.as_mut_slice(), dist);
        let nodes = nodes.into_iter().map(|(_d, n)| n).collect();
        Self { nodes }
    }

    fn find_corner_pt<'a, I, DF>(
        iter: I,
        dist: DF
    ) -> Option<usize>
    where
        'x: 'a,
        I: IntoIterator<Item = &'a P>,
        DF: Copy,
        for<'b, 'c> DF: Fn(&'b P, &'c P) -> D
    {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            let max = iter.enumerate().max_by(
                |(_, a), (_, b)| dist(&first, a).partial_cmp(&dist(&first, b)).unwrap()
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

    // Recursively build the vantage point tree
    //
    // 1. Choose the point with the largest distance to the parent as
    //    the next vantage point. The initial distances are chosen
    //    with respect to an arbitrary point, so the first vantage
    //    point is in some corner of space.
    //
    // 2. Calculate the distances of all other points to the vantage
    //    point.
    //
    // 3. Define the "inside" set as the points within less than the
    //    median distance to the vantage point, excepting the vantage
    //    point itself. The points with larger distance form the
    //    "outside" set. Build vantage point trees for each of the two
    //    sets.
    //
    fn build_tree<DF>(
        pts: &mut [(D, Node<P, D>)],
        dist: DF,
    )
    where
        DF: Copy,
        for<'a, 'b> DF: Fn(&'a P, &'b P) -> D
    {
        if pts.len() < 2 { return }
        // debug_assert!(pts.is_sorted_by_key(|pt| pt.0))
        // the last point is the one furthest away from the parent,
        // so it is the best candidate for the next vantage point
        pts.swap(0, pts.len() - 1);
        let (vp, pts) = pts.split_first_mut().unwrap();
        for (d, pt) in pts.iter_mut() {
            *d = dist(&vp.1.vantage_pt, &pt.vantage_pt)
        }
        pts.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let median_idx = pts.len() / 2;
        let (inside, outside) = pts.split_at_mut(median_idx);
        vp.1.children = Some(Children {
            radius: outside.first().unwrap().0,
            outside_offset: median_idx
        });
        Self::build_tree(inside, dist);
        Self::build_tree(outside, dist);
    }

    pub fn nearest_in<DF, Q>(&self, pt: &Q, dist: DF) -> Option<(&P, D)>
    where
        DF: Copy,
        for<'a, 'b> DF: Fn(&'a Q, &'b P) -> D
    {
        debug!("Starting nearest neighbour search");
        Self::nearest_in_subtree(self.nodes.as_slice(), pt, dist)
    }

    fn nearest_in_subtree<'a, DF, Q>(
        subtree: &'a [Node<P, D>],
        pt: &Q,
        dist: DF
    ) -> Option<(&'a P, D)>
    where
        DF: Copy,
        for<'b, 'c> DF: Fn(&'b Q, &'c P) -> D
    {
        if let Some((vp, tree)) = subtree.split_first() {
            let d = dist(pt, &vp.vantage_pt);
            let mut nearest = Some((&vp.vantage_pt, d));
            if let Some(children) = &vp.children {
                let mut subtrees = tree.split_at(children.outside_offset);
                if d > children.radius {
                    std::mem::swap(&mut subtrees.0, &mut subtrees.1);
                    trace!("Looking into outer region first");
                }
                trace!("Looking for nearest neighbour in more promising region");
                match Self::nearest_in_subtree(subtrees.0, pt, dist) {
                    res @ Some((_, dsub)) if dsub < d => {
                        nearest = res
                    },
                    _ => { }
                };
                if nearest.unwrap().1 < (children.radius - d).abs() {
                    return nearest;
                }
                trace!("Looking for nearest neighbour in less promising region");
                match Self::nearest_in_subtree(subtrees.1, pt, dist) {
                    nearest @ Some((_, dsub)) if dsub < d => {
                        nearest
                    },
                    _ => nearest
                }

            } else {
                nearest
            }
        } else {
            None
        }
    }

    pub fn k_nearest_in<DF, Q>(&self, pt: &Q, k: usize, dist: DF) -> SortedMultimap<D, &P>
    where
        DF: Copy,
        for<'a, 'b> DF: Fn(&'a Q, &'b P) -> D
    {
        debug!("Starting nearest neighbour search");
        let mut res = SortedMultimap::with_capacity(k);
        self.fill_to_k_nearest_in(pt, k, &mut res, dist);
        res
    }

    fn fill_to_k_nearest_in<'c, 'd: 'c, DF, Q>(
        &'d self,
        pt: &Q,
        k: usize,
        candidates: &mut SortedMultimap<D, &'c P>,
        dist: DF,
    )
    where
        DF: Copy,
        for<'a, 'b> DF: Fn(&'a Q, &'b P) -> D
    {
        if candidates.len() >= k {
            return;
        }
        Self::fill_to_k_nearest_in_subtree(
            self.nodes.as_slice(),
            pt,
            k,
            candidates,
            dist
        )
    }

    fn fill_to_k_nearest_in_subtree<'a, 'b: 'a, DF, Q>(
        subtree: &'b [Node<P, D>],
        pt: &Q,
        k: usize,
        found: &mut SortedMultimap<D, &'a P>,
        dist: DF,
    )
    where
        DF: Copy,
        for<'c, 'd> DF: Fn(&'c Q, &'d P) -> D
    {
        debug_assert!(found.len() <= k);
        if let Some((vp, tree)) = subtree.split_first() {
            let d = dist(pt, &vp.vantage_pt);
            if let Some(children) = &vp.children {
                let mut subtrees = tree.split_at(children.outside_offset);
                if d > children.radius {
                    trace!("Looking into outer region first");
                    std::mem::swap(&mut subtrees.0, &mut subtrees.1);
                }
                trace!("Looking for nearest neighbours in more promising region");
                Self::fill_to_k_nearest_in_subtree(subtrees.0, pt, k, found, dist);
                Self::insert_if_k_nearest(d, &vp.vantage_pt, found, k);
                if found.len() == k && found.last().unwrap().0 < (children.radius - d).abs() {
                    return;
                }
                trace!("Looking for nearest neighbours in less promising region");
                Self::fill_to_k_nearest_in_subtree(subtrees.1, pt, k, found, dist);
            } else {
                Self::insert_if_k_nearest(d, &vp.vantage_pt, found, k);
            }
        }
    }

    fn insert_if_k_nearest<'a, 'b: 'a>(
        d: D,
        pt: &'b P,
        cur_nearest: &mut SortedMultimap<D, &'a P>,
        max_size: usize,
    ) {
        if cur_nearest.len() < max_size {
            cur_nearest.insert(d, pt);
        } else {
            let replace = match cur_nearest.last() {
                Some((dsub, _)) if d < *dsub => {
                    true
                },
                Some(_) => false,
                None => unreachable!()
            };
            if replace {
                cur_nearest.pop_largest();
                cur_nearest.insert(d, pt);
            }
        }
    }

}

impl<P: Copy + Default + PartialOrd + Signed + Sub<Output = P>> Dist for P {
    type Output = Self;

    fn dist(&self, p: &P) -> Self::Output {
        (*self - *p).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::debug;

    fn log_init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn nearest() {
        log_init();

        let tree = VPTree::<i32, i32>::from_iter([]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&0), None);

        let tree = VPTree::from_iter([0]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&-1), Some((&0, 1)));
        assert_eq!(tree.nearest(&0), Some((&0, 0)));
        assert_eq!(tree.nearest(&1), Some((&0, 1)));

        let tree = VPTree::from_iter([0, 1]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&0), Some((&0, 0)));
        assert_eq!(tree.nearest(&1), Some((&1, 0)));
        assert_eq!(tree.nearest(&2), Some((&1, 1)));

        let tree = VPTree::from_iter([0, 1, 4]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&3), Some((&4, 1)));

        let tree = VPTree::from_iter([0, 1, 2, 3]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&2), Some((&2, 0)));
        assert_eq!(tree.nearest(&5), Some((&3, 2)));
        assert_eq!(tree.nearest(&-5), Some((&0, 5)));
    }

    #[test]
    fn k_nearest() {
        log_init();

        let tree = VPTree::<i32, i32>::from_iter([0, 1, 2, 3]);
        debug!("{tree:#?}");
        assert_eq!(
            tree.k_nearest(&2, 3),
            SortedMultimap::from([(0, &2), (1, &1), (1, &3)])
        )
    }

}
