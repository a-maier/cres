use std::{path::{Path, PathBuf}, convert::Infallible};

use crate::traits::Rewind;

use hepmc2::event::{Particle, CrossSection, EnergyUnit, LengthUnit, PdfInfo, Vertex};
use ntuplereader::NTupleReader;
use log::trace;

// TODO: code duplication with hepmc2 converter
const OUTGOING_STATUS: i32 = 1;
// const INCOMING_STATUS: i32 = 4;

#[derive(Debug, Default)]
pub struct Reader {
    r: NTupleReader,
    files: Vec<PathBuf>,
}

impl Reader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file<P: AsRef<Path>>(&mut self, file: P) {
        self.files.push(file.as_ref().to_owned());
        self.r.add_file(file.as_ref());
    }

    pub fn add_files<I, P>(&mut self, files: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>
    {
        for file in files {
            self.add_file(file)
        }
    }
}

impl Rewind for Reader {
    type Error = Infallible;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.r = Default::default();
        for f in &self.files {
            self.r.add_file(f)
        }
        Ok(())
    }
}

impl Iterator for Reader {
    type Item = Result<hepmc2::Event, Infallible>;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: some code duplication with hepmc2 code
        if !self.r.next_entry() {
            return None
        }

        let nparticles = self.r.get_particle_number();
        let mut particles = Vec::with_capacity(nparticles as usize);
        for i in 0..nparticles {
            let p = [
                self.r.get_energy(i),
                self.r.get_x(i),
                self.r.get_y(i),
                self.r.get_z(i),
            ];
            let p = Particle {
                id: self.r.get_pdg_code(i),
                p: hepmc2::event::FourVector(p),
                m: 0.,
                theta: theta(p),
                phi: phi(p),
                status: OUTGOING_STATUS,
                ..Default::default()
            };
            particles.push(p)
        }
        let xs = CrossSection {
            cross_section: self.r.get_cross_section(),
            cross_section_error: self.r.get_cross_section_error(),
        };
        let pdf_info = PdfInfo {
            parton_id: [self.r.get_id1() as i32, self.r.get_id2() as i32],
            x: [self.r.get_x1(), self.r.get_x2()],
            scale: self.r.get_factorization_scale(),
            ..Default::default() // TODO: xf?
        };
        let vertices = vec![Vertex {
            particles_out: particles,
            ..Default::default()
        }];
        let ev =  hepmc2::Event {
            number: self.r.get_id(),
            scale: self.r.get_renormalization_scale(),
            weights: vec![self.r.get_weight()],
            vertices,
            xs,
            pdf_info,
            energy_unit: EnergyUnit::GEV,
            length_unit: LengthUnit::MM,
            ..Default::default()
        };
        trace!("{ev:#?}");
        Some(Ok(ev))
    }
}

fn phi(p: [f64; 4]) -> f64 {
    p[1].atan2(p[2])
}

fn theta(p: [f64; 4]) -> f64 {
    pt(p).atan2(p[3])
}

fn pt2(p: [f64; 4]) -> f64 {
    p[1] * p[1] + p[2] * p[2]
}

fn pt(p: [f64; 4]) -> f64 {
    pt2(p).sqrt()
}
