use crate::c_api::event::{EventView, TypeSet};
use crate::event::Event;
use crate::traits::Distance;

use std::ffi::c_void;
use std::fmt::{self, Debug, Formatter};
use std::os::raw::c_double;

use log::trace;
use noisy_float::prelude::*;

/// User-defined distance function
#[repr(C)]
#[derive(Copy, Clone)]
pub struct DistanceFn {
    /// The distance function
    ///
    /// This has to be a *thread-safe* function that _may never return NaN_.
    /// The first argument is a pointer to the `data` member of this struct.
    /// The remaining arguments are the events for which we compute the distance.
    pub fun: unsafe fn(*mut c_void, &EventView, &EventView) -> c_double,
    /// Arbitrary data used by the distance function
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
    fn distance(&self, ev1: &Event, ev2: &Event) -> N64 {
        trace!("Compute distance between {:?} and {:?}", ev1, ev2);
        let type_sets1 = extract_typesets(ev1);
        let type_set_views1: Vec<_> =
            type_sets1.iter().map(TypeSet::view).collect();
        let event_view1 = EventView {
            id: ev1.id(),
            weights: ev1.weights.data_ptr() as *const f64,
            n_weights: ev1.n_weights(),
            type_sets: type_set_views1.as_ptr(),
            n_type_sets: type_set_views1.len(),
        };
        let type_sets2 = extract_typesets(ev2);
        let type_set_views2: Vec<_> =
            type_sets2.iter().map(TypeSet::view).collect();
        let event_view2 = EventView {
            id: ev2.id(),
            weights: ev2.weights.data_ptr() as *const f64,
            n_weights: ev2.n_weights(),
            type_sets: type_set_views2.as_ptr(),
            n_type_sets: type_set_views2.len(),
        };
        let dist = unsafe { (self.fun)(self.data, &event_view1, &event_view2) };
        n64(dist)
    }

    // In contrast with the default we assume that the supplied distance
    // function *can* compare events with different multiplicities.
    fn allows_mixed_multiplicities() -> bool {
        true
    }
}

fn extract_typesets(ev: &Event) -> Vec<TypeSet> {
    ev.outgoing()
        .iter()
        .map(|(id, p)| TypeSet {
            pid: id.id(),
            momenta: p
                .iter()
                .map(|p| [p[0].into(), p[1].into(), p[2].into(), p[3].into()])
                .collect(),
        })
        .collect()
}
