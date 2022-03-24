use std::cmp::{PartialEq, PartialOrd};
use std::default::Default;
use std::iter::FromIterator;
use std::ops::Sub;

use log::{debug, trace};
use num_traits::sign::Signed;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct VPTree<P, D> {
    nodes: Vec<Node<P, D>>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Node<P, D> {
    vantage_pt: P,
    cache: Cache<P, D>,
    children: Option<Children<D>>
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Cache<P, D> {
    pt: P,
    dist: D,
    used: bool,
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

impl<P: Dist + Copy + PartialEq> VPTree<P, <P as Dist>::Output> {
    pub fn new(nodes: Vec<P>) -> Self {
        Self::from_iter(nodes.into_iter())
    }

    pub fn nearest(&mut self, pt: &P) -> Option<(&P, <P as Dist>::Output)> {
        self.nearest_in(pt, |p, q| p.dist(q))
    }
}

impl<P: Dist + Copy + PartialEq> FromIterator<P> for VPTree<P, <P as Dist>::Output> {
    fn from_iter<I: IntoIterator<Item=P>>(iter: I) -> Self {
        Self::from_iter_with_dist(iter, |p, q| p.dist(q))
    }
}

impl<P: Copy + PartialEq, D: Copy + Default + PartialOrd + Signed + Sub> VPTree<P, D> {
    pub fn new_with_dist<DF>(
        nodes: Vec<P>,
        dist: DF
    ) -> Self
    where
        for<'a, 'b> DF: FnMut(&'a P, &'b P) -> D
    {
        Self::from_iter_with_dist(nodes.into_iter(), dist)
    }
}

impl<'x, P: Copy + PartialEq + 'x, D: Copy + Default + PartialOrd + Signed + Sub> VPTree<P, D> {
    pub fn from_iter_with_dist<DF, I>(
        iter: I,
        mut dist: DF
    ) -> Self
    where
        I: IntoIterator<Item = P>,
        for<'a, 'b> DF: FnMut(&'a P, &'b P) -> D
    {
        let mut nodes = Vec::from_iter(
            iter.into_iter().map(
                |vantage_pt| {
                    // reserve first element for storing distances
                    let cache = Cache {
                        pt: vantage_pt,
                        dist: Default::default(),
                        used: false,
                    };
                    (Default::default(), Node{ vantage_pt, children: None, cache })
                }
            )
        );
        let corner_pt_idx = Self::find_corner_pt(
            nodes.iter().map(|(_, pt)| &pt.vantage_pt),
            &mut dist
        );
        debug!("first vantage point: {corner_pt_idx:?}");
        if let Some(pos) = corner_pt_idx {
            let last_idx = nodes.len() - 1;
            nodes.swap(pos, last_idx)
        }
        Self::build_tree(nodes.as_mut_slice(), &mut dist);
        let nodes = nodes.into_iter().map(|(_d, n)| n).collect();
        Self { nodes }
    }

    fn find_corner_pt<'a, I, DF>(
        iter: I,
        dist: &mut DF
    ) -> Option<usize>
    where
        'x: 'a,
        I: IntoIterator<Item = &'a P>,
        for<'b, 'c> DF: FnMut(&'b P, &'c P) -> D
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
        dist: &mut DF,
    )
    where
        for<'a, 'b> DF: FnMut(&'a P, &'b P) -> D
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

    pub fn nearest_in<DF>(&mut self, pt: &P, mut dist: DF) -> Option<(&P, D)>
    where
        for<'a, 'b> DF: FnMut(&'a P, &'b P) -> D
    {
        debug!("Starting nearest neighbour search");
        let idx = Self::nearest_in_subtree(
            self.nodes.as_mut_slice(),
            *pt,
            &mut dist,
            0
        );
        if let Some((idx, d)) = idx {
            trace!("nearest is at index {idx}");
            self.nodes[idx].cache.used = true;
            Some((&self.nodes[idx].vantage_pt, d))
        } else {
            None
        }
    }

    fn nearest_in_subtree<'a, DF>(
        subtree: &'a mut [Node<P, D>],
        pt: P,
        dist: &mut DF,
        idx: usize,
    ) -> Option<(usize, D)>
    where
        for<'b, 'c> DF: FnMut(&'b P, &'c P) -> D,
    {
        trace!("node at position {idx}");
        if let Some((node, tree)) = subtree.split_first_mut() {
            if pt != node.cache.pt {
                node.cache = Cache{
                    pt,
                    dist: dist(&pt, &node.vantage_pt),
                    used: false
                };
            };
            let d = node.cache.dist;
            let mut nearest = if node.cache.used || pt == node.vantage_pt {
                trace!("excluding {idx}");
                None
            } else {
                Some((idx, d))
            };
            if let Some(children) = &node.children {
                let mut subtrees = tree.split_at_mut(children.outside_offset);
                let mut offsets = (1, children.outside_offset + 1);
                let mut nearest_in_sub = |sub, idx| Self::nearest_in_subtree(
                    sub,
                    pt,
                    dist,
                    idx
                );
                if d > children.radius {
                    std::mem::swap(&mut subtrees.0, &mut subtrees.1);
                    std::mem::swap(&mut offsets.0, &mut offsets.1);
                    trace!("Looking into outer region first");
                }
                trace!("Looking for nearest neighbour in more promising region");
                nearest = Self::nearer(nearest, nearest_in_sub(subtrees.0, idx + offsets.0));
                if let Some((_, dn)) = nearest {
                    if dn < (children.radius - d).abs() {
                        return nearest;
                    }
                }
                trace!("Looking for nearest neighbour in less promising region");
                Self::nearer(nearest, nearest_in_sub(subtrees.1, idx + offsets.1))
            } else {
                nearest
            }
        } else {
            None
        }
    }

    fn nearer<T>(a: Option<(T, D)>, b: Option<(T, D)>) -> Option<(T, D)> {
        match (&a, &b) {
            (&Some((_, d1)), &Some((_, d2))) => if d1 <= d2 {
                a
            } else {
                b
            },
            (&None, &Some(_)) => b,
            _ => a,
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

        let mut tree = VPTree::<i32, i32>::from_iter([]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&0), None);

        let mut tree = VPTree::from_iter([0]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&-1), Some((&0, 1)));
        assert_eq!(tree.nearest(&0), None);
        assert_eq!(tree.nearest(&1), Some((&0, 1)));

        let mut tree = VPTree::from_iter([0, 1]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&0), Some((&1, 1)));
        assert_eq!(tree.nearest(&2), Some((&1, 1)));
        assert_eq!(tree.nearest(&2), Some((&0, 2)));

        let mut tree = VPTree::from_iter([0, 1, 4]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&3), Some((&4, 1)));

        let mut tree = VPTree::from_iter([0, 1, 2, 3]);
        debug!("{tree:#?}");
        assert_eq!(tree.nearest(&2), Some((&3, 1)));
        assert_eq!(tree.nearest(&5), Some((&3, 2)));
        assert_eq!(tree.nearest(&-5), Some((&0, 5)));
    }

}
