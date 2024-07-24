use std::{
    fmt::{self, Display},
    str::FromStr,
};

use itertools::Itertools;
use jetty::{anti_kt_f, cambridge_aachen_f, kt_f, Cluster, PseudoJet};
use noisy_float::prelude::*;
use particle_id::{
    gauge_bosons::W_plus, sm_elementary_particles::{bottom, electron, gluon, muon, photon, W_minus}, ParticleID
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    event::{Event, EventBuilder},
    four_vector::FourVector,
    traits::Clustering,
};

/// Default clustering of particles into infrared safe objects
#[derive(Deserialize, Serialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct DefaultClustering {
    jet_def: JetDefinition,
    lepton_def: Option<JetDefinition>,
    photon_def: Option<PhotonDefinition>,
    reconstruct_W: bool,
    include_neutrinos: bool,
}

impl DefaultClustering {
    /// Construct a new converter using the given jet clustering
    pub fn new(jet_def: JetDefinition) -> Self {
        Self {
            jet_def,
            lepton_def: None,
            photon_def: None,
            include_neutrinos: false,
            reconstruct_W: false,
        }
    }

    /// Enable lepton clustering
    pub fn with_lepton_def(mut self, lepton_def: JetDefinition) -> Self {
        self.lepton_def = Some(lepton_def);
        self
    }

    /// Enable photon isolation
    pub fn with_photon_def(mut self, photon_def: PhotonDefinition) -> Self {
        self.photon_def = Some(photon_def);
        self
    }

    /// Whether to include neutrinos in final event record
    pub fn include_neutrinos(mut self, include: bool) -> Self {
        self.include_neutrinos = include;
        self
    }

    /// Whether to reconstruct an intermediate W boson
    #[allow(non_snake_case)]
    pub fn reconstruct_W(mut self, reconstruct: bool) -> Self {
        self.reconstruct_W = reconstruct;
        self
    }

    fn is_clustered_to_lepton(&self, id: ParticleID) -> bool {
        self.lepton_def.is_some()
            && (is_light_lepton(id.abs()) || is_photon(id))
    }

    fn is_isolated(
        &self,
        p: &FourVector,
        event: &[(ParticleID, Box<[FourVector]>)],
    ) -> bool {
        let Some(photon_def) = self.photon_def.as_ref() else {
            return false;
        };
        let photon_pt = p.pt();
        // Check photon is sufficiently hard (above min_pt)
        if photon_pt < photon_def.min_pt {
            return false;
        }
        // Check photon is sufficiently isolated
        let p = PseudoJet::from(p);
        let mut cone_mom = PseudoJet::new();
        for (e_id, particles) in event {
            // ignore neutrinos/muons in isolation cone
            if !e_id.abs().is_neutrino() && !is_muon(e_id.abs()) {
                for &ep in particles.iter() {
                    let ep = PseudoJet::from(ep);
                    if ep.delta_r(&p) < photon_def.radius {
                        cone_mom += ep;
                    }
                }
            }
        }
        // remove momentum of the original photon particle from cone
        cone_mom -= p;
        // check photon is sufficiently hard compared to surrounding cone
        let e_fraction = n64(photon_def.min_e_fraction);
        let cone_et = (cone_mom.e() * cone_mom.e()
            - cone_mom.pz() * cone_mom.pz())
        .sqrt();
        photon_pt > e_fraction * cone_et
    }

    // reconstruct intermediate W bosons
    // 1. For each type of charged lepton find the corresponding
    //    anti-neutrinos
    // 2. Take all pairs of (charged lepton, anti-neutrino)
    //    with a mass between `MW_MIN` and `MW_MAX`
    // 3. Iteratively combine the pairs with mass closest to `MW`
    //    into a W, removing all remaining pairs containing the
    //    used charged lepton or antineutrino.
    // Note that *both* the W and its decay products are included
    // in the metric
    #[allow(non_snake_case)]
    fn add_reconstructed_Ws(
        &self,
        outgoing: &[(ParticleID, Box<[FourVector]>)],
        ev: &mut EventBuilder
    ) {
        const MW_MIN: f64 = 60.;
        const MW_MAX: f64 = 100.;
        const MW: f64 = 80.377;
        let charged_leptons = outgoing.iter()
            .filter(|(kind, _)| kind.abs().is_charged_lepton());
        for (l, pl) in charged_leptons {
            let mut nu_l_bar = ParticleID::new(- l.id().abs() - 1);
            if l.is_anti_particle() {
                nu_l_bar = nu_l_bar.abs();
            };
            let nu_pos = outgoing
                .binary_search_by_key(&nu_l_bar, |&(kind, _)| kind);
            let Ok(nu_pos) = nu_pos else {
                // no corresponding (anti-)neutrinos found
                continue;
            };
            let pnu = outgoing[nu_pos].1.as_ref();
            let w_id = if l.is_anti_particle() {
                W_plus
            } else {
                W_minus
            };

            let pairs =
                pl.iter().cartesian_product(pnu)
                .filter(|&(&pl, &pnu)| {
                    let mw = (pl + pnu).m();
                    mw > MW_MIN && mw < MW_MAX
                });
            let mut pairs = Vec::from_iter(pairs);
            pairs.sort_by_key(|&(&pl, &pnu)| (n64(MW) - (pl + pnu).m()).abs());
            pairs.reverse();
            while let Some((&pl, &pnu)) = pairs.pop() {
                ev.add_outgoing(w_id, pl + pnu);
                pairs.retain(|&(&ppl, &ppnu)| pl != ppl && pnu != ppnu);
            }
        }
    }
}

impl Clustering for DefaultClustering {
    type Error = std::convert::Infallible;

    fn cluster(&self, mut orig_ev: Event) -> Result<Event, Self::Error> {
        let id = orig_ev.id;
        let weights = std::mem::take(&mut orig_ev.weights);
        let mut outgoing = orig_ev.outgoing().to_owned();
        let mut ev = EventBuilder::new();

        let mut clustered_to_leptons = Vec::new();
        let mut clustered_to_jets = Vec::new();

        if self.reconstruct_W {
            self.add_reconstructed_Ws(&outgoing, &mut ev);
        }

        // treat photons
        debug_assert!(outgoing.windows(2).all(|p| p[0].0 >= p[1].0));
        if let Ok(photon_pos) = outgoing.binary_search_by(|p| photon.cmp(&p.0))
        {
            for p in outgoing[photon_pos].1.iter() {
                if self.is_isolated(p, outgoing.as_slice()) {
                    ev.add_outgoing(PID_ISOLATED_PHOTON, *p);
                } else if self.lepton_def.is_some() {
                    clustered_to_leptons.push(p.into())
                } else {
                    ev.add_outgoing(photon, *p);
                }
            }
            outgoing.swap_remove(photon_pos);
        }

        // treat all other particles
        for (id, out) in outgoing {
            if is_parton(id) || is_hadron(id.abs()) {
                for p in out.iter() {
                    clustered_to_jets.push(p.into());
                }
            } else if self.is_clustered_to_lepton(id) {
                for p in out.iter() {
                    clustered_to_leptons.push(p.into());
                }
            } else if self.include_neutrinos || !id.abs().is_neutrino() {
                for p in out.iter() {
                    ev.add_outgoing(id, *p);
                }
            }
        }

        // add jets
        for jet in cluster(clustered_to_jets, &self.jet_def) {
            ev.add_outgoing(PID_JET, jet.into());
        }

        // add dressed leptons
        if let Some(lepton_def) = self.lepton_def.as_ref() {
            for lepton in cluster(clustered_to_leptons, lepton_def) {
                ev.add_outgoing(PID_DRESSED_LEPTON, lepton.into());
            }
        }

        let mut ev = ev.build();
        ev.weights = weights;
        ev.id = id;
        Ok(ev)
    }
}

/// Perform no clustering into IRC safe objects
#[derive(Clone, Debug)]
pub struct NoClustering {}

/// Perform no clustering into IRC safe objects
pub const NO_CLUSTERING: NoClustering = NoClustering {};

impl Clustering for NoClustering {
    type Error = std::convert::Infallible;

    fn cluster(&self, ev: Event) -> Result<Event, Self::Error> {
        Ok(ev)
    }
}

/// Placeholder for an unknown jet algorithm
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
#[derive(Deserialize, Serialize, Debug, Copy, Clone)]
pub enum JetAlgorithm {
    /// The [anti-kt](https://arxiv.org/abs/0802.1189) algorithm
    AntiKt,
    /// The [Cambridge](https://arxiv.org/abs/hep-ph/9707323)/[Aachen](https://arxiv.org/abs/hep-ph/9907280) algorithm
    CambridgeAachen,
    /// The [kt](https://arxiv.org/abs/hep-ph/9305266) algorithm
    Kt,
}

/// Definition of a jet
#[derive(Deserialize, Serialize, Debug, Copy, Clone)]
pub struct JetDefinition {
    /// Jet algorithm
    pub algorithm: JetAlgorithm,
    /// Jet radius parameter
    pub radius: f64,
    /// Minimum jet transverse momentum
    pub min_pt: f64,
}

/// Definition of an isolated object
#[derive(Deserialize, Serialize, Debug, Copy, Clone)]
pub struct PhotonDefinition {
    /// Minimum energy fraction
    pub min_e_fraction: f64,
    /// Radius parameter
    pub radius: f64,
    /// Minimum transverse momentum
    pub min_pt: f64,
}

pub(crate) fn is_parton(id: ParticleID) -> bool {
    id.id().abs() <= bottom.id() || id == gluon
}

pub(crate) fn is_hadron(id: ParticleID) -> bool {
    particle_id::hadrons::HADRONS.contains(&id.abs())
}

pub(crate) fn is_light_lepton(id: ParticleID) -> bool {
    id == electron || id == muon
}

pub(crate) fn is_photon(id: ParticleID) -> bool {
    id == photon
}

pub(crate) fn is_muon(id: ParticleID) -> bool {
    id == muon
}

pub(crate) const PID_JET: ParticleID = ParticleID::new(81);
pub(crate) const PID_DRESSED_LEPTON: ParticleID = ParticleID::new(82);
pub(crate) const PID_ISOLATED_PHOTON: ParticleID = ParticleID::new(83);

/// Cluster the given `partons` into jets
pub fn cluster(
    partons: Vec<PseudoJet>,
    jet_def: &JetDefinition,
) -> Vec<PseudoJet> {
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
