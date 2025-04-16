use std::{
    fmt::{self, Display},
    str::FromStr,
};

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
    reconstruct_W: WReconstruction,
    include_neutrinos: bool,
    min_missing_pt: f64,
}

/// How to reconstruct W bosons
#[derive(Deserialize, Serialize, Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum WReconstruction {
    /// Don't reconstruct W bosons
    #[default]
    None,
    /// Reconstruct W bosons if the reconstructed (invariant) mass matches
    ByMass,
    /// Reconstruct W bosons if the reconstructed transverse mass matches
    ByTransverseMass,
}

impl DefaultClustering {
    /// Construct a new converter using the given jet clustering
    pub fn new(jet_def: JetDefinition) -> Self {
        Self {
            jet_def,
            lepton_def: None,
            photon_def: None,
            include_neutrinos: false,
            reconstruct_W: Default::default(),
            min_missing_pt: 0.
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

    /// Whether to require a minimum missing transverse momentum
    pub fn min_missing_pt(mut self, min_missing_pt: f64) -> Self {
        self.min_missing_pt = min_missing_pt;
        self
    }

    /// Whether to reconstruct an intermediate W boson
    #[allow(non_snake_case)]
    pub fn reconstruct_W(mut self, reconstruct: WReconstruction) -> Self {
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
    // Note that *both* the W and its decay products are included
    // in the metric
    #[allow(non_snake_case)]
    fn add_reconstructed_Ws(
        &self,
        orig_outgoing: &[(ParticleID, Box<[FourVector]>)],
        ev: &mut EventBuilder
    ) {
        if self.reconstruct_W == WReconstruction::None {
            return
        }
        let mut charged_leptons = ev.outgoing()
            .iter()
            .filter(|(kind, _)| *kind == PID_DRESSED_LEPTON);
        let Some((_, pl)) = charged_leptons.next() else {
            return
        };
        assert!(charged_leptons.next().is_none());

        // let p_miss = -ev.outgoing()
        //     .iter()
        //     .filter_map(|(t, p)| if t.abs().is_neutrino() {
        //         None
        //     } else {
        //         Some(*p)
        //     })
        //     .reduce(std::ops::Add::add)
        //     .unwrap_or_default();

        let p_miss = -orig_outgoing
            .iter()
            .filter_map(|(t, p)| if t.abs().is_neutrino() {
                None
            } else {
                Some(p.iter().copied().reduce(std::ops::Add::add).unwrap_or_default())
            })
            .reduce(std::ops::Add::add)
            .unwrap_or_default();
        // reconstruct missing energy such that p_miss is lightlike
        let e_miss = p_miss.spatial_norm();
        let p_miss = FourVector::from([e_miss, p_miss[1], p_miss[2], p_miss[3]]);

        let mw_reco = (*pl + p_miss).m();
        let is_w = match self.reconstruct_W {
            WReconstruction::ByMass =>  {
                // invariant mass cuts matching Rivet's MC_WINC analysis
                const MW_MIN: f64 = 60.;
                const MW_MAX: f64 = 100.;
                mw_reco > MW_MIN && mw_reco < MW_MAX
            },
            WReconstruction::ByTransverseMass => {
                // invariant mass cut matching ATLAS_2011_I925932
                // transverse mass cut matching ATLAS, e.g. ATLAS_2011_I925932
                const MT_MIN: f64 = 40.;
                let pt_miss = FourVector::from(
                    [n64(0.), p_miss[1], p_miss[2], n64(0.)]
                );
                let dphi = PseudoJet::from(pl).delta_phi(
                    &PseudoJet::from(pt_miss)
                );
                let mt2 = n64(2.)*pl.pt()*pt_miss.pt()*(n64(1.) - dphi.cos());
                mw_reco < 1000. && mt2 > MT_MIN * MT_MIN
            },
            WReconstruction::None => unreachable!(),
        };
        if is_w {
            let mut bare_charged_leptons = orig_outgoing
                .iter()
                .filter(|(kind, _)| kind.abs().is_charged_lepton());
            let (l, pl_bare) = bare_charged_leptons.next().unwrap();
            assert_eq!(pl_bare.len(), 1);
            let pl_bare = pl_bare[0];
            assert_eq!(&pl_bare, pl);
            assert!(bare_charged_leptons.next().is_none());
            let w_id = if l.is_anti_particle() {
                W_plus
            } else {
                W_minus
            };
            let mut nu_l_bar = ParticleID::new(- l.id().abs() - 1);
            if l.is_anti_particle() {
                nu_l_bar = nu_l_bar.abs();
            };
            let (_, pnu) = orig_outgoing
                .iter()
                .find(|(t, _)|  *t == nu_l_bar)
                .unwrap();
            assert_eq!(pnu.len(), 1);
            let pnu = pnu[0];
            ev.add_outgoing(w_id, *pl + pnu);
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
        for (id, out) in &outgoing {
            if is_parton(*id) || is_hadron(id.abs()) {
                for p in out.iter() {
                    clustered_to_jets.push(p.into());
                }
            } else if self.is_clustered_to_lepton(*id) {
                for p in out.iter() {
                    clustered_to_leptons.push(p.into());
                }
            } else if self.include_neutrinos || !id.abs().is_neutrino() {
                let missing_pt = out.iter().copied()
                    .reduce(|p1, p2| p1 + p2)
                    .unwrap()
                    .pt();
                if missing_pt > self.min_missing_pt {
                    for p in out.iter() {
                        // only keep transverse momentum components
                        let p = [- p.pt() * p.pt(), p[1], p[2], n64(0.)];
                        ev.add_outgoing(*id, p.into());
                    }
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
                if lepton.rap().abs() < 2.4 {
                    ev.add_outgoing(PID_DRESSED_LEPTON, lepton.into());
                }
            }
        }

        self.add_reconstructed_Ws(&outgoing, &mut ev);

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

/// Internal particle ID for jets
pub const PID_JET: ParticleID = ParticleID::new(81);
/// Internal particle ID for dressed leptons
pub const PID_DRESSED_LEPTON: ParticleID = ParticleID::new(82);
/// Internal particle ID for isolated photons
pub const PID_ISOLATED_PHOTON: ParticleID = ParticleID::new(83);

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
