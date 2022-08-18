use crate::traits::Distance;
use crate::vptree::{NearestNeighbourIter, VPTree};

use noisy_float::prelude::*;
use rayon::prelude::*;

/// Nearest neighbour search for indexed points
pub trait NeighbourSearch<D: Distance<usize> + Send + Sync> {
    /// Iterator over nearest neighbours
    ///
    /// This has to implement `Iterator<Item = (usize, N64)>`, where
    /// the first tuple element is the index of the nearest neighbour
    /// and the second one the distance.  At the moment it is
    /// unfortunately impossible to enforce this constraint at the
    /// trait level.
    type Iter;

    /// Return nearest neighbours in order for the point with the given index
    fn nearest_in(
        self,
        point: &usize,
        d: D
    ) -> Self::Iter;
}

/// Data structure to hold information for nearest-neighbour searches
pub trait NeighbourData {
    /// Initialise nearest neighbour search
    ///
    /// The arguments are the number of points and a function
    /// returning the distance given the indices of two points
    fn new_with_dist<D>(
        npoints: usize,
        d: D,
        max_dist: N64,
    ) -> Self
    where D: Distance<usize>;
}

/// Nearest-neighbour search using a vantage point tree
pub type TreeSearch = VPTree<usize>;

impl<'a, D> NeighbourSearch<D> for &'a mut TreeSearch
where D: Distance<usize> + Send + Sync
{
    type Iter = NearestNeighbourIter<'a, usize, D>;

    fn nearest_in(
        self,
        point: &usize,
        d: D
    ) -> Self::Iter {
        self.nearest_in(point, d)
    }
}

impl NeighbourData for TreeSearch {
    fn new_with_dist<D>(
        npoints: usize,
        d: D,
        max_dist: N64
    ) -> Self
    where D: Distance<usize>
    {
        Self::from_iter_with_dist(0..npoints, d).with_max_dist(max_dist)
    }
}

/// Naive nearest neighbour search
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct NaiveNeighbourSearch {
    dist: Vec<(usize, N64)>,
    max_dist: N64,
}

impl<'a, D> NeighbourSearch<D> for &'a mut NaiveNeighbourSearch
where D: Distance<usize> + Send + Sync
{
    type Iter = NaiveNeighbourIter<'a>;

    fn nearest_in(
        self,
        point: &usize,
        d: D
    ) -> Self::Iter
    {
        let max_dist = self.max_dist;
        self.dist.par_iter_mut().for_each(|(id, dist)| {
            *dist = d.distance(id, point);
        });
        NaiveNeighbourIter::new(&self.dist, *point, max_dist)
    }
}

impl NeighbourData for NaiveNeighbourSearch {
    fn new_with_dist<D>(
        npoints: usize,
        _d: D,
        max_dist: N64,
    ) -> Self
    where D: Distance<usize>
    {
        Self {
            dist: Vec::from_iter((0..npoints).map(|id| (id, n64(0.)))),
            max_dist
        }
    }
}

#[derive(PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct NaiveNeighbourIter<'a>{
    dist: &'a [(usize, N64)],
    candidates: Vec<usize>,
    max_dist: N64,
}

impl<'a>  NaiveNeighbourIter<'a>{
    fn new(
        dist: &'a [(usize, N64)],
        seed: usize,
        max_dist: N64,
    ) -> Self {
        let mut candidates = Vec::from_iter(0..dist.len());
        candidates.swap_remove(seed);
        Self {
            dist,
            candidates,
            max_dist
        }
    }
}

impl<'a> Iterator for NaiveNeighbourIter<'a> {
    type Item = (usize, N64);

    fn next(&mut self) -> Option<Self::Item> {
        let nearest = self.candidates
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
