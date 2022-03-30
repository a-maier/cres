use crate::traits::{NeighbourData, NeighbourSearch};

use noisy_float::prelude::*;
use rayon::prelude::*;

/// Naive nearest neighbour search
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct NaiveNeighbourSearch {
    dist: Vec<(usize, N64)>
}

impl<'a> NeighbourSearch for &'a mut NaiveNeighbourSearch {
    type Iter = NaiveNeighbourIter<'a>;

    fn nearest_in<D>(
        self,
        point: &usize,
        d: D
    ) -> Self::Iter
    where
        D: Fn(&usize, &usize) -> N64 + Send + Sync
    {
        self.dist.par_iter_mut().for_each(|(id, dist)| {
            *dist = d(id, point);
        });
        NaiveNeighbourIter::new(&self.dist, *point)
    }
}

impl NeighbourData for NaiveNeighbourSearch {
    fn new_with_dist<D>(npoints: usize, _d: D) -> Self
    where D: FnMut(&usize, &usize) -> N64
    {
        Self {
            dist: Vec::from_iter((0..npoints).map(|id| (id, n64(0.))))
        }
    }
}

#[derive(PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct NaiveNeighbourIter<'a>{
    dist: &'a [(usize, N64)],
    candidates: Vec<usize>,
}

impl<'a>  NaiveNeighbourIter<'a>{
    fn new(dist: &'a [(usize, N64)], seed: usize) -> Self {
        let mut candidates = Vec::from_iter(0..dist.len());
        candidates.swap_remove(seed);
        Self {
            dist,
            candidates
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
            self.candidates.swap_remove(pos);
            Some((idx, self.dist[idx].1))
        } else {
            None
        }
    }
}
