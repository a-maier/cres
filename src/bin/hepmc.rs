use cres::event::Event;

use jetty::{anti_kt_f, cluster_if};
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
