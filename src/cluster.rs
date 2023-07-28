use std::{fmt::{Display, self}, str::FromStr};

use jetty::{Cluster, PseudoJet, anti_kt_f, kt_f, cambridge_aachen_f};
use particle_id::{ParticleID, sm_elementary_particles::{photon, electron, gluon, muon, bottom}};
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

pub(crate) fn is_parton(id: ParticleID) -> bool {
    id.id().abs() <= bottom.id() || id == gluon
}

pub(crate) fn is_hadron(id: ParticleID) -> bool {
    particle_id::hadrons::HADRONS.contains(&id)
}

pub(crate) fn is_light_lepton(id: ParticleID) -> bool {
    id == electron || id == muon
}

pub(crate) fn is_photon(id: ParticleID) -> bool {
    id == photon
}

pub(crate) const PID_JET: ParticleID = ParticleID::new(81);
pub(crate) const PID_DRESSED_LEPTON: ParticleID = ParticleID::new(82);

pub fn cluster(partons: Vec<PseudoJet>, jet_def: &JetDefinition) -> Vec<PseudoJet> {
    let minpt2 = jet_def.min_pt * jet_def.min_pt;
    let cut = |jet: PseudoJet| jet.pt2() > minpt2;
    let r = jet_def.radius;
    match jet_def.algorithm {
        JetAlgorithm::AntiKt => partons.cluster_if(anti_kt_f(r), cut),
        JetAlgorithm::Kt => partons.cluster_if(kt_f(r), cut),
        JetAlgorithm::CambridgeAachen => {
            partons.cluster_if(cambridge_aachen_f(r), cut)
        }
    }
}
