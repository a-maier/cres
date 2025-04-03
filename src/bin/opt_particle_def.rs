use clap::Parser;

use cres::cluster::WReconstruction;

use crate::opt_common::{JetDefinition, LeptonDefinition, PhotonDefinition};

#[derive(Debug, Copy, Clone, Parser)]
pub(crate) struct ParticleDefinitions {
    #[clap(flatten)]
    pub(crate) jet_def: JetDefinition,

    #[clap(flatten)]
    pub(crate) lepton_def: LeptonDefinition,

    #[clap(flatten)]
    pub(crate) photon_def: PhotonDefinition,

    /// Include neutrinos (missing pt) in the distance measure
    #[clap(long, default_value_t)]
    pub(crate) include_neutrinos: bool,

    /// Minimum missing transverse momentum
    #[clap(long, default_value_t)]
    pub(crate) min_missing_pt: f64,

    /// Reconstruct intermediate W bosons
    #[clap(long, value_parser = parse_w_reconstruction, default_value = "none")]
    pub(crate) reconstruct_W: WReconstruction,
}

fn parse_w_reconstruction(s: &str) -> Result<WReconstruction, String> {
    match s {
        "none" => Ok(WReconstruction::None),
        "by-mass" | "m" => Ok(WReconstruction::ByMass),
        "by-transverse-mass" | "mT" => Ok(WReconstruction::ByTransverseMass),
        _ => Err(format!("Value '{s}' not supported for --reconstruct-w")),
    }
}
