use cres::event::Event;

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use bzip2::read::BzDecoder;
use jetty::{anti_kt_f, cluster_if};
use hepmc2::reader::{Reader, LineParseError};
use log::info;
use noisy_float::prelude::*;

fn is_parton(particle: &hepmc2::event::Particle) -> bool {
    let id = particle.id;
    id.abs() <= 5 || id == 21
}

const OUTGOING_STATUS: i32 = 1;
const PID_JET: i32 = 81;

pub(crate) fn from(event: hepmc2::event::Event) -> Event {
    let mut res = Event::new();
    let mut partons = Vec::new();
    res.weight = n64(*event.weights.first().unwrap());
    for vx in event.vertices {
        let outgoing = vx.particles_out.into_iter().filter(
            |p| p.status == OUTGOING_STATUS
        );
        for out in outgoing {
            if is_parton(&out) {
                partons.push(out.p.0.into());
            } else {
                let p = [n64(out.p[0]), n64(out.p[1]), n64(out.p[2]), n64(out.p[3])];
                res.add_outgoing(out.id, p.into())
            }
        }
    }
    let jets = cluster_if(partons, &anti_kt_f(0.4), |jet| jet.pt2() > 400.);
    for jet in jets {
        let p = [jet.e(), jet.px(), jet.py(), jet.pz()];
        res.add_outgoing(PID_JET, p.into());
    }
    res
}

pub struct CombinedReader {
    next_files: Vec<File>,
    previous_files: Vec<File>,
    reader: Reader<Box<dyn BufRead>>
}

fn empty_reader() -> Reader<Box<dyn BufRead>> {
    Reader::new(Box::new(BufReader::new(std::io::empty())))
}

impl CombinedReader {
    pub fn new(files: Vec<File>) -> Self {
        CombinedReader{
            next_files: files,
            previous_files: Vec::new(),
            reader: empty_reader(),
        }
    }

    pub fn rewind(&mut self) -> Result<(), std::io::Error> {
        self.previous_files.reverse();
        self.next_files.append(&mut self.previous_files);
        for file in &mut self.next_files {
            file.seek(SeekFrom::Start(0))?;
        }
        self.reader = empty_reader();
        Ok(())
    }
}

impl Iterator for CombinedReader {
    type Item = Result<hepmc2::event::Event, LineParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.reader.next();
        if next.is_none() {
            if let Some(next_file) = self.next_files.pop() {
                self.previous_files.push(next_file.try_clone().unwrap());
                info!(
                    "Reading from file {}/{}",
                    self.previous_files.len(), self.previous_files.len() + self.next_files.len()
                );

                let decoder = BzDecoder::new(BufReader::new(next_file));
                let new_reader = Reader::from(
                    Box::new(BufReader::new(decoder))
                        as Box<dyn BufRead>
                );
                self.reader = new_reader;
                self.next()
            } else {
                None
            }
        } else {
            next
        }
    }
}
