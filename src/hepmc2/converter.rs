use crate::cluster::{JetDefinition, is_parton, is_light_lepton, cluster, PID_JET, is_photon, PID_DRESSED_LEPTON, is_hadron};
use crate::event::{Event, EventBuilder};
use crate::traits::TryConvert;

use hepmc2::event::EnergyUnit;
use noisy_float::prelude::*;

const OUTGOING_STATUS: i32 = 1;

/// Convert a HepMC event into internal format with jet clustering
#[derive(Copy, Clone, Debug)]
pub struct ClusteringConverter {
    jet_def: JetDefinition,
    lepton_def: Option<JetDefinition>,
}

impl ClusteringConverter {
    /// Construct a new converter using the given jet clustering
    pub fn new(jet_def: JetDefinition) -> Self {
        Self { jet_def, lepton_def: None }
    }

    /// Enable lepton clustering
    pub fn with_lepton_def(mut self, lepton_def: JetDefinition) -> Self {
        self.lepton_def = Some(lepton_def);
        self
    }

    fn is_clustered_to_lepton(&self, id: i32) -> bool {
        self.lepton_def.is_some() && (is_light_lepton(id.abs()) || is_photon(id))
    }
}

impl TryConvert<hepmc2::Event, Event> for ClusteringConverter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        event: hepmc2::Event,
    ) -> Result<Event, Self::Error> {
        let mut partons = Vec::new();
        let mut leptons = Vec::new();
        let mut builder = EventBuilder::new();
        builder.weight(n64(*event.weights.first().unwrap()));
        for vx in event.vertices {
            let outgoing = vx
                .particles_out
                .into_iter()
                .filter(|p| p.status == OUTGOING_STATUS);
            for mut out in outgoing {
                // rescale all energies to GeV
                if event.energy_unit == EnergyUnit::MEV {
                    for p in &mut out.p.0 {
                        *p /= 1000.;
                    }
                }
                let out = out;
                if is_parton(out.id) || is_hadron(out.id) {
                    partons.push(out.p.0.into());
                } else if self.is_clustered_to_lepton(out.id) {
                    leptons.push(out.p.0.into());
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
        if let Some(lepton_def) = self.lepton_def.as_ref() {
            let leptons = cluster(leptons, lepton_def);
            for lepton in leptons {
                let p = [lepton.e(), lepton.px(), lepton.py(), lepton.pz()];
                builder.add_outgoing(PID_DRESSED_LEPTON, p.into());
            }
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
