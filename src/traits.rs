use crate::cell::Cell;
use crate::event::{Event, Weights};

pub use crate::distance::Distance;
pub use crate::neighbour_search::{NeighbourSearch, NeighbourSearchAlgo};
pub use crate::seeds::SelectSeeds;

/// Update event weights
pub trait UpdateWeights {
    /// Error updating weights
    type Error;

    /// Update all event weights
    fn update_all_weights(
        &mut self,
        weights: &[Weights],
    ) -> Result<usize, Self::Error>;

    /// Update the weights for the next event
    fn update_next_weights(
        &mut self,
        weights: &Weights,
    ) -> Result<bool, Self::Error>;

    /// Finish updating weights
    fn finish_weight_update(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Rewind to the beginning of a stream
pub trait Rewind {
    /// Error during rewinding
    type Error;

    /// Rewind to the beginning of a stream
    fn rewind(&mut self) -> Result<(), Self::Error>;
}

/// Express event in terms of IRC safe objects
pub trait Clustering {
    /// Error
    type Error;

    /// Express event in terms of IRC safe objects
    fn cluster(&self, ev: Event) -> Result<Event, Self::Error>;
}

/// Convert between two types
///
/// In contrast to [std::convert::TryFrom] the converter can maintain
/// internal state.
pub trait TryConvert<From, To> {
    /// Conversion error
    type Error;

    /// Convert between two types
    fn try_convert(&self, f: From) -> Result<To, Self::Error>;
}

/// Resample events
pub trait Resample {
    /// Resampling error
    type Error;

    /// Resample events
    fn resample(&mut self, e: &[Event]) -> Result<(), Self::Error>;
}

/// Unweight events
pub trait Unweight {
    /// Unweighting error
    type Error;

    /// Unweight events
    fn unweight(&mut self, e: &[Event]) -> Result<(), Self::Error>;
}

/// Callback after resampling a cell
pub trait ObserveCell {
    /// Look at the new cell
    fn observe_cell(&self, cell: &Cell);
    /// Called after the resampling is completed
    ///
    /// For example, this can be used to write out statistics.
    /// The default is to do nothing.
    fn finish(&mut self) {}
}

/// Progress indicator, e.g. a progress bar
pub trait Progress {
    /// Advance the progress by `i`
    fn inc(&self, i: u64);
    /// Signal that we are done
    fn finish(&self);
}
