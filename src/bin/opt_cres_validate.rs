use thiserror::Error;

use crate::{opt_common::{LeptonDefinition, PhotonDefinition}, opt_cres::Opt};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Error)]
pub(crate) enum ValidationError {
    #[error("Either all or none of --leptonalgorithm, --leptonradius, --leptonpt have to be set")]
    BadLeptonOpt,
    #[error("Either all or none of --photonefrac, --photonradius, --photonpt have to be set")]
    BadPhotonOpt,
}

pub(crate) fn validate(opt: Opt) -> Result<Opt, ValidationError> {
    let &LeptonDefinition {
        leptonalgorithm,
        leptonpt,
        leptonradius,
    } = &opt.lepton_def;
    let &PhotonDefinition {
        photonefrac,
        photonradius,
        photonpt,
    } = &opt.photon_def;
    match (leptonalgorithm, leptonpt, leptonradius) {
        (Some(_), Some(_), Some(_)) => Ok(()),
        (None, None, None) => Ok(()),
        _ => Err(ValidationError::BadLeptonOpt),
    }?;
    match (photonefrac, photonradius, photonpt) {
        (Some(_), Some(_), Some(_)) => Ok(()),
        (None, None, None) => Ok(()),
        _ => Err(ValidationError::BadPhotonOpt),
    }?;
    Ok(opt)
}
