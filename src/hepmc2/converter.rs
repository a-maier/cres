use std::fmt::{self, Display};
use std::str::FromStr;

use crate::event::{Event, EventBuilder};
use crate::traits::TryConvert;

use jetty::{anti_kt_f, cambridge_aachen_f, cluster_if, kt_f, PseudoJet};
use noisy_float::prelude::*;
use thiserror::Error;

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

/// Jet clustering algorithms
#[derive(Debug, Copy, Clone)]
pub enum JetAlgorithm {
    /// The [anti-kt](https://arxiv.org/abs/0802.1189) algorithm
    AntiKt,
    /// The [Cambridge](https://arxiv.org/abs/hep-ph/9707323)/[Aachen](https://arxiv.org/abs/hep-ph/9907280) algorithm
    CambridgeAachen,
    /// The [kt](https://arxiv.org/abs/hep-ph/9305266) algorithm
    Kt,
}

#[derive(Debug, Copy, Clone)]
pub struct JetDefinition {
    /// Jet algorithm
    pub algorithm: JetAlgorithm,
    /// Jet radius parameter
    pub radius: f64,
    /// Minimum jet transverse momentum
    pub min_pt: f64,
}

fn is_parton(particle: &hepmc2::event::Particle) -> bool {
    let id = particle.id;
    id.abs() <= 5 || id == 21
}

const OUTGOING_STATUS: i32 = 1;
const PID_JET: i32 = 81;

fn cluster(partons: Vec<PseudoJet>, jet_def: &JetDefinition) -> Vec<PseudoJet> {
    let minpt2 = jet_def.min_pt * jet_def.min_pt;
    let cut = |jet: PseudoJet| jet.pt2() > minpt2;
    let r = jet_def.radius;
    match jet_def.algorithm {
        JetAlgorithm::AntiKt => cluster_if(partons, &anti_kt_f(r), cut),
        JetAlgorithm::Kt => cluster_if(partons, &kt_f(r), cut),
        JetAlgorithm::CambridgeAachen => {
            cluster_if(partons, &cambridge_aachen_f(r), cut)
        }
    }
}

/// Convert a HepMC event into internal format with jet clustering
#[derive(Copy, Clone, Debug)]
pub struct ClusteringConverter {
    jet_def: JetDefinition,
}

impl ClusteringConverter {
    /// Construct a new converter using the given jet clustering
    pub fn new(jet_def: JetDefinition) -> Self {
        Self { jet_def }
    }
}

impl TryConvert<(hepmc2::Event, EventBuilder), Event> for ClusteringConverter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        ev: (hepmc2::Event, EventBuilder),
    ) -> Result<Event, Self::Error> {
        let mut partons = Vec::new();
        let (event, mut builder) = ev;
        builder.weight(n64(*event.weights.first().unwrap()));
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
                    builder.add_outgoing(out.id, p.into());
                }
            }
        }
        let jets = cluster(partons, &self.jet_def);
        for jet in jets {
            let p = [jet.e(), jet.px(), jet.py(), jet.pz()];
            builder.add_outgoing(PID_JET, p.into());
        }
        Ok(builder.build())
    }
}

/// Straightforward conversion of HepMC events to internal format
#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Converter {}

impl Converter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TryConvert<(hepmc2::Event, EventBuilder), Event> for Converter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        ev: (hepmc2::Event, EventBuilder),
    ) -> Result<Event, Self::Error> {
        let (event, mut builder) = ev;
        builder.weight(n64(*event.weights.first().unwrap()));
        for vx in event.vertices {
            let outgoing = vx
                .particles_out
                .into_iter()
                .filter(|p| p.status == OUTGOING_STATUS);
            for out in outgoing {
                let p = [
                    n64(out.p[0]),
                    n64(out.p[1]),
                    n64(out.p[2]),
                    n64(out.p[3]),
                ];
                builder.add_outgoing(out.id, p.into());
            }
        }
        Ok(builder.build())
    }
}
