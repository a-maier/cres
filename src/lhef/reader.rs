use std::cmp::min;
use std::io::{Error, ErrorKind, BufReader, BufRead, Seek};
use std::fmt::{Debug, Display};

use hepmc2::Event;
use hepmc2::event::{Particle, Vertex, CrossSection};
use lhef::{HEPEUP, HEPRUP};

use crate::auto_decompress::auto_decompress;
use crate::file::File;
use crate::reader::{RewindError, EventReadError};
use crate::traits::{Rewind, TryClone};

pub struct FileReader {
    reader: ::lhef::Reader<Box<dyn BufRead>>,
    source: File,
}

impl FileReader {
    pub fn new(source: File) -> Result<Self, std::io::Error> {
        let cloned_source = source.try_clone()?;
        let input = auto_decompress(BufReader::new(cloned_source));
        let reader = ::lhef::Reader::new(input).map_err(
            |err| create_error(&source, err)
        )?;
        Ok(FileReader {
            source,
            reader
        })
    }
}

impl Rewind for FileReader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        use RewindError::*;
        self.source.rewind()?;
        let cloned_source = self.source.try_clone().map_err(CloneError)?;
        let input = auto_decompress(BufReader::new(cloned_source));
        self.reader = ::lhef::Reader::new(input).map_err(
            |err| create_error(&self.source, err)
        )?;

        Ok(())
    }
}

impl Iterator for FileReader {
    type Item = Result<Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.hepeup()
            .transpose()
            .map(|r| match r{
                Ok(hepeup) => Ok(into_event(self.reader.heprup(), hepeup)),
                Err(err) => Err(err.into()),
            })
    }
}

// convert to HepMC event
fn into_event(
    heprup: &HEPRUP,
    hepeup: HEPEUP
) -> Event {
    // vertex id most be any non-positive number according to HepMC standard?
    const VTX_ID: i32 = -1;

    const LHEF_INCOMING: i32 = -1;
    const LHEF_OUTGOING: i32 = 1;

    const HEPMC_INCOMING: i32 = 4;
    const HEPMC_OUTGOING: i32 = 1;

    assert!(hepeup.NUP >= 0);
    let nparticles = hepeup.NUP as usize;
    let mut incoming = Vec::with_capacity(2);
    let mut outgoing = Vec::with_capacity(min(2, nparticles) - 2);
    for i in 0..nparticles {
        let p = hepeup.PUP[i];
        let p = [p[3], p[0], p[1], p[2]];
        let mut p = Particle {
            id: hepeup.IDUP[i],
            p: hepmc2::event::FourVector(p),
            m: hepeup.PUP[i][4],
            ..Default::default()
        };
        match hepeup.ISTUP[i] {
            LHEF_INCOMING => {
                p.status = HEPMC_INCOMING;
                p.end_vtx = VTX_ID;
                incoming.push(p)
            },
            LHEF_OUTGOING => {
                p.status = HEPMC_OUTGOING;
                outgoing.push(p)
            },
            _ => { } // ignore intermediate particles
        }
    }
    let vertices = vec![Vertex {
        particles_in: incoming,
        particles_out: outgoing,
        barcode: VTX_ID,
        ..Default::default()
    }];
    let xs_err = heprup.XERRUP.iter()
        .map(|e| e * e)
        .sum::<f64>()
        .sqrt();
    let xs = CrossSection{
        cross_section: heprup.XSECUP.iter().sum(),
        cross_section_error: xs_err,
    };
    Event {
        number: hepeup.IDRUP,
        scale: hepeup.SCALUP,
        alpha_qcd: hepeup.AQCDUP,
        alpha_qed: hepeup.AQEDUP,
        weights: vec![hepeup.XWGTUP],
        vertices,
        xs,
        ..Default::default()
    }
}

fn create_error(
    file: impl Debug,
    err: impl Display
) -> Error {
    Error::new(
        ErrorKind::Other,
        format!("Failed to create LHEF reader for {file:?}: {err}")
    )
}
