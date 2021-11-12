use crate::event::Event;

use rayon::prelude::*;

/// Select seed events
pub trait SelectSeeds {
    /// The output type
    ///
    /// This has to implement `Iterator<Item = usize>`. At the moment
    /// it is unfortunately impossible to enforce this constraint at
    /// the trait level.
    type Iter;

    /// Select seeds for cell construction from the given event.
    ///
    /// The return value should be an iterator over the indices of the
    /// seeds in `events`, in the order in which cells are to be
    /// constructed.
    fn select_seeds(&mut self, events: &[Event]) -> Self::Iter;
}

/// Strategy for seeds selection
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Strategy {
    /// Select events with the negative weight closest to zero first
    LeastNegative,
    /// Select events with the most negative weight first
    MostNegative,
    /// Take negative-weight events in the order passed to `select_seeds`
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
    strategy: Strategy
}

impl StrategicSelector {
    /// Select event seeds according to the given [Strategy]
    pub fn new(strategy: Strategy) -> Self {
        Self { strategy }
    }
}

impl SelectSeeds for StrategicSelector {
    type Iter = std::vec::IntoIter<usize>;

    fn select_seeds(&mut self, events: &[Event]) -> Self::Iter {
        use Strategy::*;
        let mut neg_weight: Vec<_> = events.par_iter().enumerate().filter(
            |(_n, e)| e.weight < 0.
        ).map(|(n, _e)| n).collect();
        match self.strategy {
            Next => {},
            MostNegative => neg_weight.par_sort_unstable_by_key(|&n| events[n].weight),
            LeastNegative => neg_weight
                .par_sort_unstable_by(|&n, &m| events[m].weight.cmp(&events[n].weight)),
        }
        neg_weight.into_iter()
    }
}
