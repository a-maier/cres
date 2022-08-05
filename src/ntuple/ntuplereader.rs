use std::ffi::CStr;
use std::{ffi::CString, path::Path};
use std::os::unix::ffi::OsStrExt;

#[derive(Debug)]
pub struct NTupleReader (
    *mut nTupleReader
);

impl Default for NTupleReader {
    fn default() -> Self {
        Self(unsafe { ntuple_reader_new() } )
    }
}

impl NTupleReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_tree<T: AsRef<str>>(name: T) -> Self {
        let name = CString::new(name.as_ref()).unwrap();
        Self( unsafe { ntuple_reader_from_tree(name.as_ptr()) } )
    }

    pub fn next_entry(&mut self) -> bool {
        unsafe { next_entry(self.0) }
    }

    pub fn set_pdf<T: AsRef<str>>(&mut self, name: T) {
        let name = CString::new(name.as_ref()).unwrap();
        unsafe { set_pdf(self.0, name.as_ptr()) }
    }

    pub fn set_pdf_member(&mut self, member: i32) {
        unsafe { set_pdf_member(self.0, member.into()) }
    }

    pub fn get_id(&mut self) -> i32 {
        unsafe { get_id(self.0) }.into()
    }

    pub fn get_particle_number(&mut self) -> i32 {
        unsafe { get_particle_number(self.0) }.into()
    }

    pub fn get_energy(&mut self, i: i32) -> f64 {
        unsafe { get_energy(self.0, i.into()) }
    }

    pub fn get_x(&mut self, i: i32) -> f64 {
        unsafe { get_x(self.0, i.into()) }
    }

    pub fn get_y(&mut self, i: i32) -> f64 {
        unsafe { get_y(self.0, i.into()) }
    }

    pub fn get_z(&mut self, i: i32) -> f64 {
        unsafe { get_z(self.0, i.into()) }
    }

    pub fn get_pdg_code(&mut self, i: i32) -> i32 {
        unsafe { get_pdg_code(self.0, i.into()) }
    }

    pub fn get_x1(&mut self) -> f64 {
        unsafe { get_x1(self.0) }
    }

    pub fn get_x2(&mut self) -> f64 {
        unsafe { get_x2(self.0) }
    }

    pub fn get_id1(&mut self) -> f64 {
        unsafe { get_id1(self.0) }
    }

    pub fn get_id2(&mut self) -> f64 {
        unsafe { get_id2(self.0) }
    }

    pub fn get_alphas_power(&mut self) -> i16 {
        unsafe { get_alphas_power(self.0).into() }
    }

    pub fn get_renormalization_scale(&mut self) -> f64 {
        unsafe { get_renormalization_scale(self.0) }
    }

    pub fn get_factorization_scale(&mut self) -> f64 {
        unsafe { get_factorization_scale(self.0) }
    }

    pub fn get_weight(&mut self) -> f64 {
        unsafe { get_weight(self.0) }
    }

    pub fn get_weight2(&mut self) -> f64 {
        unsafe { get_weight2(self.0) }
    }

    pub fn get_me_weight(&mut self) -> f64 {
        unsafe { get_me_weight(self.0) }
    }

    pub fn get_me_weight2(&mut self) -> f64 {
        unsafe { get_me_weight2(self.0) }
    }

    pub fn get_type(&mut self) -> i8 {
        unsafe { get_type(self.0).into() }
    }

    pub fn compute_weight(
        &mut self,
        new_factorization_scale: f64,
        new_renormalization_scale: f64,
    ) -> f64 {
        unsafe { compute_weight(self.0, new_factorization_scale, new_renormalization_scale) }
    }

    pub fn compute_weight2(
        &mut self,
        new_factorization_scale: f64,
        new_renormalization_scale: f64,
    ) -> f64 {
        unsafe { compute_weight2(self.0, new_factorization_scale, new_renormalization_scale) }
    }

    pub fn set_pp(&mut self) {
        unsafe { set_pp(self.0) }
    }

    pub fn set_ppbar(&mut self) {
        unsafe { set_pp(self.0) }
    }

    pub fn add_file<T: AsRef<Path>>(&mut self, name: T) {
        unsafe {
            let name = CStr::from_bytes_with_nul_unchecked(name.as_ref().as_os_str().as_bytes());
            add_file(self.0, name.as_ptr())
        }
    }

    // pub fn set_cms_energy(&mut self, cms_energy: f64) {
    //     unsafe { set_cms_energy(self.0, cms_energy) }
    // }

    // pub fn set_collider_type(&mut self, ct: ColliderType) {
    //     unsafe { set_collider_type(self.0, ct) }
    // }

    pub fn reset_cross_section(&mut self) {
        unsafe { reset_cross_section(self.0) }
    }

    pub fn get_cross_section(&mut self) -> f64 {
        unsafe { get_cross_section(self.0) }
    }

    pub fn get_cross_section_error(&mut self) -> f64 {
        unsafe { get_cross_section_error(self.0) }
    }
}

impl Drop for NTupleReader {
    fn drop(&mut self) {
        unsafe { drop_ntuple_reader(self.0) }
    }
}

include!(concat!(env!("OUT_DIR"), "/ntuplereader.rs"));
