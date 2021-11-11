use crate::c_api::event::{EventView, TypeSet};
use crate::event::Event;
use crate::traits::Distance;

use std::ffi::c_void;
use std::os::raw::c_double;
use std::fmt::{self, Debug, Formatter};

use noisy_float::prelude::*;
use log::trace;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DistanceFn {
    pub fun: unsafe fn(*mut c_void, &EventView, &EventView) -> c_double,
    pub data: *mut c_void,
}

impl Debug for DistanceFn {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let addr = self.fun as *const ();
        f.debug_struct("DistanceFn")
            .field("fun", &addr)
            .field("data", &self.data)
            .finish()
    }
}

unsafe impl Send for DistanceFn {}
unsafe impl Sync for DistanceFn {}

impl Distance for DistanceFn {
    fn distance(
        &self,
        ev1: &Event,
        ev2: &Event,
    ) -> N64 {
        trace!("Compute distance between {:?} and {:?}", ev1, ev2);
        let type_sets1 = extract_typesets(ev1);
        let type_set_views1: Vec<_> = type_sets1.iter().map(TypeSet::view).collect();
        let event_view1 = EventView {
            id: ev1.id(),
            weight: ev1.weight.into(),
            type_sets: type_set_views1.as_ptr(),
            n_type_sets: type_set_views1.len(),
        };
        let type_sets2 = extract_typesets(ev2);
        let type_set_views2: Vec<_> = type_sets2.iter().map(TypeSet::view).collect();
        let event_view2 = EventView {
            id: ev2.id(),
            weight: ev2.weight.into(),
            type_sets: type_set_views2.as_ptr(),
            n_type_sets: type_set_views2.len(),
        };
        let dist = unsafe {
            (self.fun)(self.data, &event_view1, &event_view2)
        };
        n64(dist)
    }
}

fn extract_typesets(ev: &Event) -> Vec<TypeSet> {
    ev.outgoing().iter().map(
        |(id, p)| TypeSet{
            pid: *id,
            momenta: p.iter().map(
                |p| [p[0].into(), p[1].into(), p[2].into(), p[3].into()]
            ).collect()
        }
    ).collect()
}
