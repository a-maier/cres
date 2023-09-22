use std::cmp::Ordering;

use noisy_float::prelude::*;
use serde::{Deserialize, Serialize};

use crate::traits::Distance;

/// A space partitioning based on a vantage-point tree
#[derive(Deserialize, Serialize)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VPTreePartition<P, DF> {
    vp: Vec<VPBisection<P>>,
    dist: DF,
}

impl<P, DF: Distance<P>> VPTreePartition<P, DF> {
    /// Construct a vantage point tree partitioning with the given
    /// bisections and distance measure
    ///
    /// `vp` is interpreted as a tree, where the children of the node
    /// with index n are at the indices 2*n + 1 (inside region) and
    /// 2*n + 2 (outside region).
    ///
    /// # Safety
    ///
    /// - The tree has to be balanced, i.e. there have to be 2^n - 1
    ///   nodes.
    /// - For a parent node with index n, the child at index 2*n + 1
    ///   should lie in the inside region. In particular, the distance
    ///   should be less than r. The child at index 2*n + 2 should lie
    ///   in the outside region with a distance larger than r.
    /// - The radii should result from `dist`.
    pub unsafe fn from_vp(vp: Vec<VPBisection<P>>, dist: DF) -> Self {
        assert!((vp.len() + 1).is_power_of_two());
        Self { vp, dist }
    }

    /// Index of the region containing the given point
    pub fn region(&self, point: &P) -> usize {
        let vp = &self.vp;
        let mut idx = 0;
        while idx < vp.len() {
            let r = vp[idx].r;
            match self.dist.distance(&vp[idx].pt, point).cmp(&r) {
                Ordering::Less | Ordering::Equal=> idx = 2 * idx + 1,
                Ordering::Greater => idx = 2 * idx + 2,
            }
        }
        idx - vp.len()
    }

    /// Consume the partition and return the vantage point tree
    ///
    /// The children of the node with index n are at the
    /// indices 2*n + 1 (inside region) and 2*n + 2 (outside region)
    pub fn into_tree(self) -> Vec<VPBisection<P>> {
        self.vp
    }

    /// Number of regions
    pub fn len(&self) -> usize {
        self.vp.len() + 1
    }
}

/// A bisection of space defined by a vantage point
#[derive(Deserialize, Serialize)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VPBisection<P> {
    /// The vantage point (centre of a hypersphere)
    pub pt: P,
    /// Radius of the hypersphere
    pub r: N64,
}
