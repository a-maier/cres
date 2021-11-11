use std::marker::PhantomData;
use std::os::raw::c_double;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct EventView<'a> {
    pub id: usize,
    pub weight: c_double,
    pub type_sets: *const TypeSetView<'a>,
    pub n_type_sets: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct TypeSetView<'a> {
    pub pid: i32,
    pub momenta: *const FourMomentum,
    pub n_momenta: usize,
    pub phantom: PhantomData<&'a ()>,
}

#[derive(Clone, Debug)]
pub struct TypeSet {
    pub pid: i32,
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

pub type FourMomentum = [c_double; 4];
