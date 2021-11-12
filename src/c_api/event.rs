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
    /// Event weight
    pub weight: c_double,
    /// Sets of particles of a given type
    pub type_sets: *const TypeSetView<'a>,
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
    pub(crate) fn view<'a>(&'a self) -> TypeSetView<'a> {
        TypeSetView {
            pid: self.pid,
            momenta: self.momenta.as_ptr(),
            n_momenta: self.momenta.len(),
            phantom: PhantomData
        }
    }
}

/// Four-momentum [E, px, py, pz]
pub type FourMomentum = [c_double; 4];
