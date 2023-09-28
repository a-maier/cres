use crate::cell::Cell;
use crate::event::{Event, Weights};

pub use crate::distance::Distance;
pub use crate::neighbour_search::{NeighbourSearchAlgo, NeighbourSearch};
pub use crate::seeds::SelectSeeds;

/// Update event weights
pub trait UpdateWeights {
    /// Error updating weights
    type Error;

    /// Update all event weights
    fn update_all_weights(&mut self, weights: &[Weights]) -> Result<usize, Self::Error>;

    /// Update the weights for the next event
    fn update_next_weights(&mut self, weights: &Weights) -> Result<bool, Self::Error>;
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
    fn resample(&mut self, e: Vec<Event>) -> Result<Vec<Event>, Self::Error>;
}

/// Unweight events
pub trait Unweight {
    /// Unweighting error
    type Error;

/// Unweight events
    fn unweight(&mut self, e: Vec<Event>) -> Result<Vec<Event>, Self::Error>;
}

/// Write events to some output
///
/// When using the [Cres](crate::cres::Cres) class, the Reader originally
/// used to read the events is passed alongside after a [Rewind]. The
/// events are guaranteed to be ordered according to their
/// [id](crate::event::Event::id). Apart from ill-behaved user-defined
/// conversions, this means they are ordered in the original way they
/// were read. Apart from its weight, the `Event` with `id == 0`
/// should correspond to the first event returned by the Reader. This
/// makes it possible to reconstruct information that is not kept
/// internally.
pub trait Write<Reader> {
    /// Write error
    type Error;

    /// Write events to some output
    fn write(&mut self, r: &mut Reader, e: &[Event])
        -> Result<(), Self::Error>;
}

/// Write a single event
pub trait WriteEvent<Ev> {
    /// Write error
    type Error;

    /// Write an event
    fn write(&mut self, e: Ev) -> Result<(), Self::Error>;

    /// Wrap up (optional)
    fn finish(self) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        Ok(())
    }
}

/// Try to clone this object
///
/// This trait is similar to [std::clone::Clone], but is allowed to fail.
pub trait TryClone {
    /// Clone error
    type Error;

    /// Try to clone this object
    fn try_clone(&self) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

impl<T: Clone> TryClone for T {
    type Error = std::convert::Infallible;

    fn try_clone(&self) -> Result<Self, Self::Error> {
        Ok(self.clone())
    }
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
