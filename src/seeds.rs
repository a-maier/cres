use crate::event::Event;

use rayon::prelude::*;

/// Select seed events
pub trait SelectSeeds {
    /// The output type
    ///
    /// This has to implement `ParallelIterator<Item = usize>`. At the moment
    /// it is unfortunately impossible to enforce this constraint at
    /// the trait level.
    type ParallelIter;

    /// Select seeds for cell construction from the given event.
    ///
    /// The return value should be an iterator over the indices of the
    /// seeds in `events`, in the order in which cells are to be
    /// constructed.
    fn select_seeds(&self, events: &[Event]) -> Self::ParallelIter;
}

/// Strategy for seeds selection
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Strategy {
    /// Select events with the negative weight closest to zero first
    LeastNegative,
    /// Select events with the most negative weight first
    MostNegative,
    /// Take negative-weight events in the order passed to [select_seeds](SelectSeeds::select_seeds)
    Next,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::MostNegative
    }
}

/// Select event seeds according to a [Strategy]
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct StrategicSelector {
    strategy: Strategy,
}

impl StrategicSelector {
    /// Select event seeds according to the given [Strategy]
    pub fn new(strategy: Strategy) -> Self {
        Self { strategy }
    }
}

impl SelectSeeds for StrategicSelector {
    type ParallelIter = rayon::iter::MaxLen<rayon::vec::IntoIter<usize>>;

    fn select_seeds(&self, events: &[Event]) -> Self::ParallelIter {
        use Strategy::*;
        let mut neg_weight: Vec<_> = events
            .par_iter()
            .enumerate()
            .filter(|(_n, e)| e.weight() < 0.)
            .map(|(n, _e)| n)
            .collect();
        match self.strategy {
            Next => {}
            MostNegative => {
                neg_weight.par_sort_unstable_by_key(|&n| events[n].weight())
            }
            LeastNegative => neg_weight.par_sort_unstable_by(|&n, &m| {
                events[m].weight().cmp(&events[n].weight())
            }),
        }
        // limit size of tasks
        // if we don't do this, the resampling can stall with only a
        // single thread trying to work on a last huge chunk of cells
        const MAX_TASK_SIZE: usize = 64;
        neg_weight.into_par_iter().with_max_len(MAX_TASK_SIZE)
    }
}
