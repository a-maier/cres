use crate::event::Event;
use crate::cell::Cell;

pub use crate::distance::Distance;

pub trait Rewind {
    type Error;

    fn rewind(&mut self) -> Result<(), Self::Error>;
}

pub trait TryConvert<From, To> {
    type Error;

    fn try_convert(&mut self, f: From) -> Result<To, Self::Error>;
}

pub trait Resample {
    type Error;

    fn resample(&mut self, e: Vec<Event>) -> Result<Vec<Event>, Self::Error>;
}

pub trait Unweight {
    type Error;

    fn unweight(&mut self, e: Vec<Event>) -> Result<Vec<Event>, Self::Error>;
}

pub trait Write<Reader> {
    type Error;

    fn write(&mut self, r: &mut Reader, e: &[Event]) -> Result<(), Self::Error>;
}

pub trait TryClone {
    type Error;

    fn try_clone(&self) -> Result<Self, Self::Error> where Self: Sized;
}

impl<T: Clone> TryClone for T {
    type Error = std::convert::Infallible;

    fn try_clone(&self) -> Result<Self, Self::Error> {
        Ok(self.clone())
    }
}

pub trait CellObserve {
    fn cell_observe(&mut self, cell: &Cell);

    fn finish(&mut self) { }
}
