use std::fmt::{self, Display};
use std::str::FromStr;

use crate::event::Event;
use crate::distance::pt_norm_sq;
use crate::traits::TryConvert;

use jetty::{anti_kt_f, cambridge_aachen_f, cluster_if, kt_f, PseudoJet};
use noisy_float::prelude::*;
use thiserror::Error;

#[derive(Debug, Copy, Clone)]
pub enum JetAlgorithm {
    AntiKt,
    CambridgeAachen,
    Kt,
}

#[derive(Debug, Clone, Error)]
pub struct UnknownJetAlgorithm(String);

impl Display for UnknownJetAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown jet algorithm: {}", self.0)
    }
}

impl FromStr for JetAlgorithm {
    type Err = UnknownJetAlgorithm;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anti_kt" | "antikt" | "anti-kt" => Ok(Self::AntiKt),
            "kt" => Ok(Self::Kt),
            "Cambridge/Aachen" | "Cambridge-Aachen" | "Cambridge_Aachen"
            | "cambridge/aachen" | "cambridge-aachen" | "cambridge_aachen" => {
                Ok(Self::CambridgeAachen)
            }
            _ => Err(UnknownJetAlgorithm(s.to_string())),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct JetDefinition {
    /// Jet algorithm
    pub jetalgorithm: JetAlgorithm,
    /// Jet radius parameter
    pub jetradius: f64,
    /// Minimum jet transverse momentum
    pub jetpt: f64,
}

fn is_parton(particle: &hepmc2::event::Particle) -> bool {
    let id = particle.id;
    id.abs() <= 5 || id == 21
}

const OUTGOING_STATUS: i32 = 1;
const PID_JET: i32 = 81;

fn cluster(partons: Vec<PseudoJet>, jet_def: &JetDefinition) -> Vec<PseudoJet> {
    let minpt2 = jet_def.jetpt * jet_def.jetpt;
    let cut = |jet: PseudoJet| jet.pt2() > minpt2;
    let r = jet_def.jetradius;
    match jet_def.jetalgorithm {
        JetAlgorithm::AntiKt => cluster_if(partons, &anti_kt_f(r), cut),
        JetAlgorithm::Kt => cluster_if(partons, &kt_f(r), cut),
        JetAlgorithm::CambridgeAachen => {
            cluster_if(partons, &cambridge_aachen_f(r), cut)
        }
    }
}

pub struct HepMCConverter {
    jet_def: JetDefinition,
    ptweight: N64
}

impl HepMCConverter {
    pub fn new(jet_def: JetDefinition, ptweight: N64) -> Self {
        Self{jet_def, ptweight}
    }
}

impl TryConvert<hepmc2::Event, Event> for HepMCConverter {
    type Error = std::convert::Infallible;

    fn try_convert(&mut self, event: hepmc2::Event) -> Result<Event, Self::Error> {
        let mut res = Event::new();
        let mut partons = Vec::new();
        res.weight = n64(*event.weights.first().unwrap());
        for vx in event.vertices {
            let outgoing = vx
                .particles_out
                .into_iter()
                .filter(|p| p.status == OUTGOING_STATUS);
            for out in outgoing {
                if is_parton(&out) {
                    partons.push(out.p.0.into());
                } else {
                    let p = [
                        n64(out.p[0]),
                        n64(out.p[1]),
                        n64(out.p[2]),
                        n64(out.p[3]),
                    ];
                    res.add_outgoing(out.id, p.into())
                }
            }
        }
        let jets = cluster(partons, &self.jet_def);
        for jet in jets {
            let p = [jet.e(), jet.px(), jet.py(), jet.pz()];
            res.add_outgoing(PID_JET, p.into());
        }
        for (_type, ps) in &mut res.outgoing_by_pid {
            ps.sort_unstable_by_key(|p| pt_norm_sq(p, self.ptweight));
            ps.reverse()
        }
        Ok(res)
    }

}
