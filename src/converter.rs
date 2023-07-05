use crate::cluster::{JetDefinition, is_parton, is_light_lepton, cluster, PID_JET, is_photon, PID_DRESSED_LEPTON, is_hadron};
use crate::event::{Event, EventBuilder};
use crate::traits::TryConvert;

use avery::event::Status;
use noisy_float::prelude::*;
use particle_id::ParticleID;

/// Convert an input event into internal format with jet clustering
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

    fn is_clustered_to_lepton(&self, id: ParticleID) -> bool {
        self.lepton_def.is_some() && (is_light_lepton(id.abs()) || is_photon(id))
    }
}

impl TryConvert<avery::Event, Event> for ClusteringConverter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        event: avery::Event,
    ) -> Result<Event, Self::Error> {
        let mut partons = Vec::new();
        let mut leptons = Vec::new();
        let mut builder = EventBuilder::new();
        let weight = event.weights.first().unwrap().weight.unwrap();
        builder.weight(n64(weight));
        let outgoing = event.particles.into_iter().filter(
            |p| p.status == Some(Status::Outgoing)
        );
        for out in outgoing {
            let id = out.id.unwrap();
            let p = out.p.unwrap();
            if is_parton(id) || is_hadron(id.abs()) {
                partons.push(p.into());
            } else if self.is_clustered_to_lepton(id) {
                leptons.push(p.into());
            } else {
                let p = [
                    n64(p[0]),
                    n64(p[1]),
                    n64(p[2]),
                    n64(p[3]),
                ];
                builder.add_outgoing(id, p.into());
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

impl TryConvert<avery::Event, Event> for Converter {
    type Error = std::convert::Infallible;

    fn try_convert(
        &mut self,
        event: avery::Event,
    ) -> Result<Event, Self::Error> {
        let mut builder = EventBuilder::new();
        let weight = event.weights.first().unwrap().weight.unwrap();
        builder.weight(n64(weight));
        let outgoing = event.particles.into_iter().filter(
            |p| p.status == Some(Status::Outgoing)
        );
        for out in outgoing {
            let p = out.p.unwrap();
            let p = [
                n64(p[0]),
                n64(p[1]),
                n64(p[2]),
                n64(p[3]),
            ];
            builder.add_outgoing(out.id.unwrap(), p.into());
        }
        Ok(builder.build())
    }
}
