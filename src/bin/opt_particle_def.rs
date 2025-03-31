use clap::Parser;

use crate::opt_common::{JetDefinition, LeptonDefinition, PhotonDefinition};

#[derive(Debug, Copy, Clone, Parser)]
pub(crate) struct ParticleDefinitions {
    #[clap(flatten)]
    pub(crate) jet_def: JetDefinition,

    #[clap(flatten)]
    pub(crate) lepton_def: LeptonDefinition,

    #[clap(flatten)]
    pub(crate) photon_def: PhotonDefinition,

    /// Include neutrinos in the distance measure
    #[clap(long, default_value_t)]
    pub(crate) include_neutrinos: bool,
}
