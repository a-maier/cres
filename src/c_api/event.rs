use crate::{
    event::{Event, EventBuilder},
    ParticleID,
};

use std::marker::PhantomData;
use std::os::raw::c_double;

/// View into an event
///
/// Changing any of the members does not change the original event.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct EventView<'a> {
    /// Event id
    pub id: usize,
    /// Event weights
    pub weights: *const c_double,
    /// Sets of particles of a given type
    pub type_sets: *const TypeSetView<'a>,
    /// Number of event weights
    pub n_weights: usize,
    /// Number of particle sets
    pub n_type_sets: usize,
}

/// View into a particle set
///
/// Changing any member does not change the original particle set
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct TypeSetView<'a> {
    /// Particle id
    ///
    /// This is usually the id according to the
    /// [PDG Monte Carlo Particle Numbering Scheme](https://pdg.lbl.gov/2021/mcdata/mc_particle_id_contents.html)
    pub pid: i32,
    /// Four-momenta of the particles
    pub momenta: *const FourMomentum,
    /// Number of particles
    pub n_momenta: usize,
    phantom: PhantomData<&'a ()>,
}

/// A set of particles with the same type
#[derive(Clone, Debug)]
pub struct TypeSet {
    /// Particle id
    ///
    /// This is usually the id according to the
    /// [PDG Monte Carlo Particle Numbering Scheme](https://pdg.lbl.gov/2021/mcdata/mc_particle_id_contents.html)
    pub pid: i32,
    /// Four-momenta of the particles
    pub momenta: Vec<FourMomentum>,
}

impl TypeSet {
    pub fn view(&self) -> TypeSetView<'_> {
        TypeSetView {
            pid: self.pid,
            momenta: self.momenta.as_ptr(),
            n_momenta: self.momenta.len(),
            phantom: PhantomData,
        }
    }
}

/// Four-momentum [E, px, py, pz]
pub type FourMomentum = [c_double; 4];

/// Convert from [EventView] to [Event]
///
/// # Safety
///
/// The input has to be valid, i.e. `weights` should point to a slice
/// with at least `n_weights` elements and `type_sets` to a slice with
/// at least `n_type_set` elements.
///
impl<'a> From<EventView<'a>> for Event {
    fn from(view: EventView<'a>) -> Self {
        use crate::n64;
        let EventView {
            id,
            weights,
            type_sets,
            n_weights,
            n_type_sets,
        } = view;
        let n_particles = (0..n_type_sets)
            .map(|n| unsafe { (*type_sets.add(n)).n_momenta })
            .sum();
        let mut event = EventBuilder::with_capacity(n_particles);
        for n_weight in 0..n_weights {
            let weight = unsafe { *weights.add(n_weight) };
            event.add_weight(n64(weight));
        }
        for n_set in 0..n_type_sets {
            let TypeSetView {
                pid,
                momenta,
                n_momenta,
                ..
            } = unsafe { *type_sets.add(n_set) };
            for n_p in 0..n_momenta {
                let p = unsafe { *momenta.add(n_p) };
                event.add_outgoing(ParticleID::new(pid), p.map(n64).into());
            }
        }
        let mut event = event.build();
        event.id = id;
        event
    }
}
