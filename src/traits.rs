use crate::cell::Cell;
use crate::event::Event;

pub use crate::distance::Distance;
pub use crate::seeds::SelectSeeds;

use noisy_float::prelude::*;

/// Rewind to the beginning of a stream
pub trait Rewind {
    type Error;

    fn rewind(&mut self) -> Result<(), Self::Error>;
}

/// Convert between two types
///
/// In contrast to [std::convert::TryFrom] the converter can maintain
/// internal state.
pub trait TryConvert<From, To> {
    type Error;

    fn try_convert(&mut self, f: From) -> Result<To, Self::Error>;
}

/// Resample events
pub trait Resample {
    type Error;

    fn resample(&mut self, e: Vec<Event>) -> Result<Vec<Event>, Self::Error>;
}

/// Unweight events
pub trait Unweight {
    type Error;

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
    type Error;

    fn write(&mut self, r: &mut Reader, e: &[Event])
        -> Result<(), Self::Error>;
}

/// Try to clone this object
///
/// This trait is similar to [std::clone::Clone], but is allowed to fail.
pub trait TryClone {
    type Error;

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
    fn nearest_in<D>(
        self,
        point: &usize,
        d: D
    ) -> Self::Iter
    where
        D: Fn(&usize, &usize) -> N64 + Send + Sync;
}

/// Data structure to hold information for nearest-neighbour searches
pub trait NeighbourData {
    /// Initialise nearest neighbour search
    ///
    /// The arguments are the number of points and a function
    /// returning the distance given the indices of two points
    fn new_with_dist<D>(npoints: usize, d: D) -> Self
    where D: Fn(&usize, &usize) -> N64;
}
