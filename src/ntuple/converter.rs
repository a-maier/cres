use crate::{traits::TryConvert, event::Event};

#[derive(Copy, Clone, Default, Debug)]
pub struct NTupleConverter { }

impl TryConvert<Event, Event> for NTupleConverter {
    type Error = std::convert::Infallible;

    fn try_convert(&mut self, e: Event) -> Result<Event, Self::Error> {
        Ok(e)
    }
}
