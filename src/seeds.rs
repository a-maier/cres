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

/// Weight criterion for choosing seeds
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum WeightSign {
    /// Select events with negative weight as seeds
    #[default]
    Negative,
    /// Select events with positive weight as seeds
    Positive,
    /// Select all events as cell seeds, regardless of weight
    All,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::MostNegative
    }
}

/// Select event seeds
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct StrategicSelector {
    strategy: Strategy,
    weight_sign: WeightSign,
}

impl StrategicSelector {
    /// Select event seeds with the given [WeightSign] in the order defined by a [Strategy]
    pub fn new(weight_sign: WeightSign, strategy: Strategy) -> Self {
        Self {
            weight_sign,
            strategy,
        }
    }
}

impl SelectSeeds for StrategicSelector {
    type ParallelIter = rayon::iter::MaxLen<rayon::vec::IntoIter<usize>>;

    fn select_seeds(&self, events: &[Event]) -> Self::ParallelIter {
        use Strategy::*;
        let filter: fn(&Event) -> bool = match self.weight_sign {
            WeightSign::Negative => |e| e.weight() < 0.,
            WeightSign::Positive => |e| e.weight() > 0.,
            WeightSign::All => |_| true,
        };
        let mut seeds: Vec<_> = events
            .par_iter()
            .enumerate()
            .filter(|(_n, e)| filter(e))
            .map(|(n, _e)| n)
            .collect();
        match self.strategy {
            Next => {}
            MostNegative => {
                seeds.par_sort_unstable_by_key(|&n| events[n].weight())
            }
            LeastNegative => seeds.par_sort_unstable_by(|&n, &m| {
                events[m].weight().cmp(&events[n].weight())
            }),
        }
        // limit size of tasks
        // if we don't do this, the resampling can stall with only a
        // single thread trying to work on a last huge chunk of cells
        const MAX_TASK_SIZE: usize = 64;
        seeds.into_par_iter().with_max_len(MAX_TASK_SIZE)
    }
}
