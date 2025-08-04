use std::cmp::{PartialEq, PartialOrd};
use std::collections::{HashMap, HashSet, VecDeque};
use std::default::Default;
use std::hash::Hash;
use std::iter::{FromIterator, Iterator};

use log::{debug, trace};
use noisy_float::prelude::*;
use rayon::prelude::*;

use crate::partition::{VPBisection, VPTreePartition};
use crate::traits::Distance;

/// A vantage-point tree
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct VPTree<P, DF> {
    nodes: Vec<Node<P>>,
    dist: DF,
    max_dist: N64,
}

impl<P, DF: Default> Default for VPTree<P, DF> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            max_dist: n64(f64::MAX),
            dist: Default::default(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Node<P> {
    vantage_pt: P,
    children: Option<Children>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct Children {
    radius: N64,
    outside_offset: usize,
}

impl<P, DF> VPTree<P, DF> {
    /// Set a maximum distance for nearest-neighbour searches
    pub fn with_max_dist(mut self, max_dist: N64) -> Self {
        self.max_dist = max_dist;
        self
    }

    /// Convert into a `Vec` of nodes and the distance function
    pub fn into_nodes_dist(self) -> (Vec<P>, DF) {
        let Self { nodes, dist, .. } = self;
        let nodes = nodes
            .into_iter()
            .map(
                |Node {
                     vantage_pt,
                     children: _,
                 }| vantage_pt,
            )
            .collect();
        (nodes, dist)
    }

    /// Access the distance function
    pub fn dist(&self) -> &DF {
        &self.dist
    }

    /// Get maximum allowed distance for nearest neighbours
    pub fn max_dist(&self) -> N64 {
        self.max_dist
    }
}

impl<P: Copy + PartialEq + Eq, DF: Distance<P>> VPTree<P, DF> {
    /// Construct a vantage-point tree without parallelisation
    pub fn seq_new(nodes: Vec<P>, dist: DF) -> Self {
        Self::from_iter_with_dist(nodes, dist)
    }
}

impl<P, DF> VPTree<P, DF>
where
    P: Copy + PartialEq + Eq + Send + Sync,
    DF: Distance<P> + Send + Sync,
{
    /// Construct a vantage-point tree from the given nodes and distance
    pub fn new(nodes: Vec<P>, dist: DF) -> Self {
        Self::par_new(nodes, dist)
    }

    /// Construct a vantage-point tree with parallelisation
    pub fn par_new(nodes: Vec<P>, dist: DF) -> Self {
        Self::from_par_iter_with_dist(nodes.into_par_iter(), dist)
    }
}

impl<'a, P, DF> VPTree<P, DF>
where
    P: Copy + PartialEq + 'a,
    DF: Distance<P>,
{
    /// Construct a vantage-point tree from the given nodes and distance
    pub fn from_iter_with_dist<I>(iter: I, dist: DF) -> Self
    where
        I: IntoIterator<Item = P>,
    {
        Self::from_iter_with_dist_and_depth(iter, dist, usize::MAX)
    }

    /// Construct the first `depth` layers of a vantage-point tree
    pub fn from_iter_with_dist_and_depth<I>(
        iter: I,
        dist: DF,
        depth: usize,
    ) -> Self
    where
        I: IntoIterator<Item = P>,
    {
        let mut nodes = Vec::from_iter(iter.into_iter().map(|vantage_pt| {
            // reserve first element for storing distances
            (
                Default::default(),
                Node {
                    vantage_pt,
                    children: None,
                },
            )
        }));
        let corner_pt_idx = Self::find_corner_pt(
            nodes.iter().map(|(_, pt)| &pt.vantage_pt),
            &dist,
        );
        debug!("first vantage point: {corner_pt_idx:?}");
        if let Some(pos) = corner_pt_idx {
            let last_idx = nodes.len() - 1;
            nodes.swap(pos, last_idx)
        }
        Self::build_tree(nodes.as_mut_slice(), &dist, depth);
        let nodes = nodes.into_iter().map(|(_d, n)| n).collect();
        Self {
            nodes,
            dist,
            max_dist: n64(f64::MAX),
        }
    }

    fn find_corner_pt<I>(iter: I, dist: &DF) -> Option<usize>
    where
        I: IntoIterator<Item = &'a P>,
        DF: Distance<P>,
    {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            let max = iter
                .enumerate()
                .max_by_key(|(_, a)| dist.distance(first, a));
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
    fn build_tree(pts: &mut [(N64, Node<P>)], dist: &DF, depth: usize) {
        if pts.len() < 2 || depth == 0 {
            return;
        }
        debug_assert!(pts.is_sorted_by_key(|pt| pt.0));
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
            outside_offset: median_idx,
        });
        Self::build_tree(inside, dist, depth - 1);
        Self::build_tree(outside, dist, depth - 1);
    }
}

impl<'a, P, DF> VPTree<P, DF>
where
    P: Copy + PartialEq + 'a + Send + Sync,
    DF: Distance<P> + Send + Sync,
{
    /// Construct a vantage-point tree from the given nodes and distance
    pub fn from_par_iter_with_dist<I>(iter: I, dist: DF) -> Self
    where
        I: ParallelIterator<Item = P>,
    {
        Self::from_par_iter_with_dist_and_depth(iter, dist, usize::MAX)
    }

    /// Construct the first `depth` layers of a vantage-point tree
    pub fn from_par_iter_with_dist_and_depth<I>(
        iter: I,
        dist: DF,
        depth: usize,
    ) -> Self
    where
        I: ParallelIterator<Item = P>,
    {
        let mut nodes: Vec<_> = iter
            .map(|vantage_pt| {
                // reserve first element for storing distances
                (
                    Default::default(),
                    Node {
                        vantage_pt,
                        children: None,
                    },
                )
            })
            .collect();

        let corner_pt_idx = if let Some((first, nodes)) = nodes.split_first() {
            Some(Self::par_find_corner_pt(
                &first.1.vantage_pt,
                nodes.par_iter().map(|(_, pt)| &pt.vantage_pt).enumerate(),
                &dist,
            ))
        } else {
            None
        };

        debug!("first vantage point: {corner_pt_idx:?}");
        if let Some(pos) = corner_pt_idx {
            let last_idx = nodes.len() - 1;
            nodes.swap(pos, last_idx)
        }
        Self::par_build_tree(nodes.as_mut_slice(), &dist, depth);
        let nodes = nodes.into_par_iter().map(|(_d, n)| n).collect();
        Self {
            nodes,
            dist,
            max_dist: n64(f64::MAX),
        }
    }

    fn par_find_corner_pt<I>(first: &P, iter: I, dist: &DF) -> usize
    where
        I: ParallelIterator<Item = (usize, &'a P)>,
    {
        let max = iter.max_by_key(|(_, a)| dist.distance(first, a));
        if let Some((pos, _)) = max {
            pos + 1
        } else {
            0
        }
    }

    fn par_build_tree(pts: &mut [(N64, Node<P>)], dist: &DF, depth: usize) {
        const PAR_MIN_SIZE: usize = 1_000;
        if depth == 0 {
            return;
        }
        if pts.len() < PAR_MIN_SIZE {
            return Self::build_tree(pts, dist, depth);
        }
        debug_assert!(pts.is_sorted_by_key(|pt| pt.0));
        pts.swap(0, pts.len() - 1);
        let (vp, pts) = pts.split_first_mut().unwrap();
        pts.par_iter_mut().for_each(|(d, pt)| {
            *d = dist.distance(&vp.1.vantage_pt, &pt.vantage_pt)
        });
        pts.par_sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let median_idx = pts.len() / 2;
        let (inside, outside) = pts.split_at_mut(median_idx);
        vp.1.children = Some(Children {
            radius: outside.first().unwrap().0,
            outside_offset: median_idx,
        });
        [inside, outside]
            .into_par_iter()
            .for_each(|region| Self::par_build_tree(region, dist, depth - 1));
    }
}

impl<P: Copy + Hash + Eq, DF: Distance<P>> VPTree<P, DF> {
    /// Find the nearest neighbours for `pt`
    pub fn nearest_in(&self, pt: &P) -> NearestNeighbourIter<'_, P, DF>
    where
        DF: Distance<P>,
    {
        NearestNeighbourIter {
            tree: self,
            pt: *pt,
            exclude: HashSet::new(),
            distance_cache: HashMap::new(),
        }
    }

    fn nearest_in_impl(
        &self,
        pt: &P,
        exclude: &HashSet<P>,
        cached_dist: &mut HashMap<P, N64>,
    ) -> Option<(P, N64)>
    where
        DF: Distance<P>,
    {
        debug!("Starting nearest neighbour search");
        let idx = self.nearest_in_subtree(
            self.nodes.as_slice(),
            *pt,
            0,
            exclude,
            cached_dist,
        );
        if let Some((idx, d)) = idx {
            trace!("nearest is at index {idx}");
            if d <= self.max_dist {
                Some((self.nodes[idx].vantage_pt, d))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn nearest_in_subtree(
        &self,
        subtree: &[Node<P>],
        pt: P,
        idx: usize,
        exclude: &HashSet<P>,
        cached_dist: &mut HashMap<P, N64>,
    ) -> Option<(usize, N64)>
    where
        DF: Distance<P>,
    {
        let dist = &self.dist;
        trace!("node at position {idx}");
        if let Some((node, tree)) = subtree.split_first() {
            let d = *cached_dist
                .entry(node.vantage_pt)
                .or_insert_with(|| dist.distance(&pt, &node.vantage_pt));
            let mut nearest = if pt == node.vantage_pt
                || exclude.contains(&node.vantage_pt)
            {
                trace!("excluding {idx}");
                None
            } else {
                Some((idx, d))
            };
            if let Some(children) = &node.children {
                let mut subtrees = tree.split_at(children.outside_offset);
                let mut offsets = (1, children.outside_offset + 1);
                if d > children.radius {
                    std::mem::swap(&mut subtrees.0, &mut subtrees.1);
                    std::mem::swap(&mut offsets.0, &mut offsets.1);
                    trace!("Looking into outer region first");
                }
                trace!(
                    "Looking for nearest neighbour in more promising region"
                );
                let nearest_pref = self.nearest_in_subtree(
                    subtrees.0,
                    pt,
                    idx + offsets.0,
                    exclude,
                    cached_dist,
                );
                nearest = Self::nearer(nearest, nearest_pref);
                let possibly_in_less_promising =
                    (d - children.radius).abs() <= self.max_dist;
                if !possibly_in_less_promising {
                    return nearest;
                }
                if let Some((_, dn)) = nearest {
                    if dn < (children.radius - d).abs() {
                        return nearest;
                    }
                }
                trace!(
                    "Looking for nearest neighbour in less promising region"
                );
                let nearest_other = self.nearest_in_subtree(
                    subtrees.1,
                    pt,
                    idx + offsets.1,
                    exclude,
                    cached_dist,
                );
                Self::nearer(nearest, nearest_other)
            } else {
                nearest
            }
        } else {
            None
        }
    }

    fn nearer<T>(a: Option<(T, N64)>, b: Option<(T, N64)>) -> Option<(T, N64)> {
        match (&a, &b) {
            (&Some((_, d1)), &Some((_, d2))) => {
                if d1 <= d2 {
                    a
                } else {
                    b
                }
            }
            (&None, &Some(_)) => b,
            _ => a,
        }
    }
}

/// Iterator over nearest neighbours
pub struct NearestNeighbourIter<'a, P: Hash + Eq, DF> {
    pt: P,
    tree: &'a VPTree<P, DF>,
    exclude: HashSet<P>,
    distance_cache: HashMap<P, N64>,
}

impl<P: Hash + Eq, DF> Iterator for NearestNeighbourIter<'_, P, DF>
where
    P: Copy + PartialEq,
    DF: Distance<P>,
{
    type Item = (P, N64);

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.tree.nearest_in_impl(
            &self.pt,
            &self.exclude,
            &mut self.distance_cache,
        );
        if let Some((pt, _)) = res {
            trace!("Excluding from further searches");
            self.exclude.insert(pt);
        }
        res
    }
}

impl<P: Default, DF: Distance<P>> From<VPTree<P, DF>>
    for VPTreePartition<P, DF>
{
    fn from(source: VPTree<P, DF>) -> Self {
        let VPTree {
            mut nodes,
            dist,
            max_dist: _,
        } = source;
        let mut vp = Vec::new();
        if nodes.is_empty() {
            // Safety: an empty partitioning is always safe
            return unsafe { Self::from_vp(vp, dist) };
        }
        let mut node_queue = VecDeque::from([0]);
        while let Some(node_idx) = node_queue.pop_front() {
            let node = std::mem::take(&mut nodes[node_idx]);
            if let Some(children) = node.children {
                let pt = node.vantage_pt;
                let r = children.radius;
                vp.push(VPBisection { pt, r });
                let inside_idx = node_idx + 1;
                node_queue.push_back(inside_idx);
                node_queue.push_back(inside_idx + children.outside_offset);
            }
        }
        // Safety: by construction of the vantage-point tree
        unsafe { Self::from_vp(vp, dist) }
    }
}
