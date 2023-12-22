// cell resampling with user-defined distance
// run with `cargo run --release --example user_distance -- IN.hepmc OUT.hepmc`
// set the environment variable `RUST_LOG=info` for command-line output
use std::error::Error;

use cres::distance::Distance;
use cres::event::Event;
use cres::prelude::*;

use env_logger;
use noisy_float::prelude::*;

// this distance is just for demonstration
// and doesn't make much sense physically
struct MyDistance {
    e_fact: N64,
}

impl Distance for MyDistance {
    fn distance(&self, ev1: &Event, ev2: &Event) -> N64 {
        if ev1.outgoing().len() != ev2.outgoing().len() {
            return N64::infinity();
        }
        let mut dist = n64(0.);
        let set_pairs = ev1.outgoing().iter().zip(ev2.outgoing().iter());
        for ((id1, s1), (id2, s2)) in set_pairs {
            if id1 != id2 || s1.len() != s2.len() {
                return N64::infinity();
            }
            dist += s1
                .iter()
                .zip(s2.iter())
                .map(|(p1, p2)| {
                    self.e_fact * (p1[0] - p2[0]).abs() + (p1[1] - p2[1]).abs()
                })
                .sum::<N64>();
        }
        n64(0.)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut args = std::env::args();
    let _ = args.next().unwrap(); // ignore program name
    let infile = args.next().unwrap().into();
    let outfile = args.next().unwrap().into();

    let event_io = IOBuilder::default();
    let event_io = event_io.build_from_files(infile, outfile)?;

    let converter = Converter::new();

    let resampler = ResamplerBuilder::default()
        .distance(MyDistance { e_fact: n64(0.5) })
        .build();

    let mut cres = CresBuilder {
        event_io,
        converter,
        clustering: NO_CLUSTERING, // disable jet clustering
        resampler,
        unweighter: NO_UNWEIGHTING, // disable unweighting
    }
    .build();
    cres.run()?;
    Ok(())
}
