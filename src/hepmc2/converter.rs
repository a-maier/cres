use crate::cluster::{JetDefinition, is_parton, cluster, PID_JET};
use crate::event::{Event, EventBuilder};
use crate::traits::TryConvert;

use noisy_float::prelude::*;

const OUTGOING_STATUS: i32 = 1;

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

impl TryConvert<hepmc2::Event, Event> for ClusteringConverter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        event: hepmc2::Event,
    ) -> Result<Event, Self::Error> {
        let mut partons = Vec::new();
        let mut builder = EventBuilder::new();
        builder.weight(n64(*event.weights.first().unwrap()));
        for vx in event.vertices {
            let outgoing = vx
                .particles_out
                .into_iter()
                .filter(|p| p.status == OUTGOING_STATUS);
            for out in outgoing {
                if is_parton(out.id) {
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

impl TryConvert<hepmc2::Event, Event> for Converter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        event: hepmc2::Event,
    ) -> Result<Event, Self::Error> {
        let mut builder = EventBuilder::new();
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
