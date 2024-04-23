use crate::traits::Distance;
use crate::vptree::{NearestNeighbourIter, VPTree};

use noisy_float::prelude::*;
use rayon::prelude::*;

/// Algorithm for nearest-neighbour search
pub trait NeighbourSearchAlgo {
    /// Data structure to hold information for nearest-neighbour searches
    type Output<D>;

    /// Initialise nearest neighbour search
    ///
    /// The arguments are the number of points and a function
    /// returning the distance given the indices of two points
    fn new_with_dist<D: Distance<usize> + Send + Sync>(
        npoints: usize,
        d: D,
        max_dist: N64,
    ) -> Self::Output<D>;
}

/// Nearest neighbour search for indexed points
pub trait NeighbourSearch {
    /// Iterator over nearest neighbours
    ///
    /// This has to implement `Iterator<Item = (usize, N64)>`, where
    /// the first tuple element is the index of the nearest neighbour
    /// and the second one the distance.  At the moment it is
    /// unfortunately impossible to enforce this constraint at the
    /// trait level.
    type Iter;

    /// Return nearest neighbours in order for the point with the given index
    fn nearest(self, point: &usize) -> Self::Iter;
}

/// Nearest-neighbour search using a vantage point tree
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TreeSearch {}

impl<'a, D> NeighbourSearch for &'a VPTree<usize, D>
where
    D: Distance<usize>,
{
    type Iter = NearestNeighbourIter<'a, usize, D>;

    fn nearest(self, point: &usize) -> Self::Iter {
        self.nearest_in(point)
    }
}

impl NeighbourSearchAlgo for TreeSearch {
    type Output<D> = VPTree<usize, D>;

    fn new_with_dist<D: Distance<usize> + Send + Sync>(
        npoints: usize,
        d: D,
        max_dist: N64,
    ) -> VPTree<usize, D> {
        let range = (0..npoints).into_par_iter();
        VPTree::from_par_iter_with_dist(range, d).with_max_dist(max_dist)
    }
}

/// Naive nearest neighbour search
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NaiveNeighbourSearch {}

/// Data required for naive nearest neighbour search
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct NaiveSearchData<D> {
    dist: D,
    cached_dist: Vec<(usize, N64)>,
    max_dist: N64,
}

impl<D: Distance<usize> + Send + Sync> NeighbourSearch for &NaiveSearchData<D> {
    type Iter = NaiveNeighbourIter;

    fn nearest(self, point: &usize) -> Self::Iter {
        let max_dist = self.max_dist;
        let mut dist = self.cached_dist.clone();
        dist.par_iter_mut().for_each(|(id, dist)| {
            *dist = self.dist.distance(id, point);
        });
        NaiveNeighbourIter::new(dist, *point, max_dist)
    }
}

impl NeighbourSearchAlgo for NaiveNeighbourSearch {
    type Output<D> = NaiveSearchData<D>;

    fn new_with_dist<D: Distance<usize> + Send + Sync>(
        npoints: usize,
        dist: D,
        max_dist: N64,
    ) -> Self::Output<D> {
        NaiveSearchData {
            dist,
            cached_dist: Vec::from_iter((0..npoints).map(|id| (id, n64(0.)))),
            max_dist,
        }
    }
}

/// Iterator over nearest neighbours using a naive search algorithm
#[derive(PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct NaiveNeighbourIter {
    dist: Vec<(usize, N64)>,
    candidates: Vec<usize>,
    max_dist: N64,
}

impl NaiveNeighbourIter {
    fn new(dist: Vec<(usize, N64)>, seed: usize, max_dist: N64) -> Self {
        let mut candidates = Vec::from_iter(0..dist.len());
        candidates.swap_remove(seed);
        Self {
            dist,
            candidates,
            max_dist,
        }
    }
}

impl Iterator for NaiveNeighbourIter {
    type Item = (usize, N64);

    fn next(&mut self) -> Option<Self::Item> {
        let nearest = self
            .candidates
            .par_iter()
            .enumerate()
            .min_by_key(|(_pos, &idx)| self.dist[idx].1);
        if let Some((pos, &idx)) = nearest {
            let dist = self.dist[idx].1;
            if dist <= self.max_dist {
                self.candidates.swap_remove(pos);
                Some((idx, self.dist[idx].1))
            } else {
                None
            }
        } else {
            None
        }
    }
}
