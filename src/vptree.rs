use std::cmp::{PartialEq, PartialOrd};
use std::default::Default;
use std::iter::{Iterator, FromIterator};

use log::{debug, trace};
use noisy_float::prelude::*;

use crate::traits::Distance;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct VPTree<P> {
    nodes: Vec<Node<P>>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Node<P> {
    vantage_pt: P,
    cache: Cache<P>,
    children: Option<Children>
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Cache<P> {
    pt: P,
    dist: N64,
    used: bool,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Children {
    radius: N64,
    outside_offset: usize,
}

impl<P: Copy + PartialEq> VPTree<P> {
    pub fn new<DF>(
        nodes: Vec<P>,
        dist: DF
    ) -> Self
    where
        DF: Distance<P>
    {
        Self::from_iter_with_dist(nodes.into_iter(), dist)
    }
}

impl<'x, P: Copy + PartialEq + 'x> VPTree<P> {
    pub fn from_iter_with_dist<DF, I>(
        iter: I,
        dist: DF
    ) -> Self
    where
        I: IntoIterator<Item = P>,
        DF: Distance<P>
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
            & dist
        );
        debug!("first vantage point: {corner_pt_idx:?}");
        if let Some(pos) = corner_pt_idx {
            let last_idx = nodes.len() - 1;
            nodes.swap(pos, last_idx)
        }
        Self::build_tree(nodes.as_mut_slice(), & dist);
        let nodes = nodes.into_iter().map(|(_d, n)| n).collect();
        Self { nodes }
    }

    fn find_corner_pt<'a, I, DF>(
        iter: I,
        dist: & DF
    ) -> Option<usize>
    where
        'x: 'a,
        I: IntoIterator<Item = &'a P>,
        DF: Distance<P>
    {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            let max = iter.enumerate().max_by_key(
                |(_, a)| dist.distance(&first, a)
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
        pts: &mut [(N64, Node<P>)],
        dist: & DF,
    )
    where
        DF: Distance<P>
    {
        if pts.len() < 2 { return }
        // debug_assert!(pts.is_sorted_by_key(|pt| pt.0))
        // the last point is the one furthest away from the parent,
        // so it is the best candidate for the next vantage point
        pts.swap(0, pts.len() - 1);
        let (vp, pts) = pts.split_first_mut().unwrap();
        for (d, pt) in pts.iter_mut() {
            *d = dist.distance(&vp.1.vantage_pt, &pt.vantage_pt)
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

    pub fn nearest_in<DF>(&mut self, pt: &P, dist: DF) -> NearestNeighbourIter<'_, P, DF>
    where
        DF: Distance<P>
    {
        NearestNeighbourIter{
            tree: self,
            pt: *pt,
            dist
        }
    }

    fn nearest_in_impl<DF>(&mut self, pt: &P, dist: DF) -> Option<(P, N64)>
    where
        DF: Distance<P>
    {
        debug!("Starting nearest neighbour search");
        let idx = Self::nearest_in_subtree(
            self.nodes.as_mut_slice(),
            *pt,
            & dist,
            0
        );
        if let Some((idx, d)) = idx {
            trace!("nearest is at index {idx}");
            self.nodes[idx].cache.used = true;
            Some((self.nodes[idx].vantage_pt, d))
        } else {
            None
        }
    }

    fn nearest_in_subtree<'a, DF>(
        subtree: &'a mut [Node<P>],
        pt: P,
        dist: &DF,
        idx: usize,
    ) -> Option<(usize, N64)>
    where
        DF: Distance<P>
    {
        trace!("node at position {idx}");
        if let Some((node, tree)) = subtree.split_first_mut() {
            if pt != node.cache.pt {
                node.cache = Cache{
                    pt,
                    dist: dist.distance(&pt, &node.vantage_pt),
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
                let nearest_in_sub = |sub, idx| Self::nearest_in_subtree(
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

    fn nearer<T>(a: Option<(T, N64)>, b: Option<(T, N64)>) -> Option<(T, N64)> {
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

pub struct NearestNeighbourIter<'a, P, DF> {
    pt: P,
    dist: DF,
    tree: &'a mut VPTree<P>
}

impl<'a, P, DF> Iterator for NearestNeighbourIter<'a, P, DF>
where
    P: Copy + PartialEq,
    DF: Distance<P>,
{
    type Item = (P, N64);

    fn next(&mut self) -> Option<Self::Item> {
        self.tree.nearest_in_impl(&self.pt, &self.dist)
    }
}