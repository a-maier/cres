use crate::traits::{Distance, NeighbourData, NeighbourSearch};
use crate::vptree::{VPTree, NearestNeighbourIter};

impl<'a, D> NeighbourSearch<D> for &'a mut VPTree<usize>
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

impl NeighbourData for VPTree<usize> {
    fn new_with_dist<D>(npoints: usize, d: D) -> Self
    where D: Distance<usize>
    {
        Self::from_iter_with_dist(0..npoints, d)
    }
}
