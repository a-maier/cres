use crate::event::Event;
use crate::traits::Unweight;

use log::warn;
use noisy_float::prelude::*;
use rand::{
    distr::{Distribution, Uniform},
    Rng,
};
use rayon::prelude::*;

/// Standard unweighter
pub struct Unweighter<R> {
    min_wt: f64,
    rng: R,
}

impl<R> Unweighter<R> {
    /// Construct new unweighter for events with weight < `min_wt`
    pub fn new(min_wt: f64, rng: R) -> Self {
        Self { min_wt, rng }
    }
}

impl<R: Rng> Unweight for Unweighter<R> {
    type Error = std::convert::Infallible;

    /// Unweight events
    ///
    /// Any event with weight |w| < `min_wt` is discarded with probability
    /// 1 - |w| / `min_wt` and reweighted to |w| = `min_wt` otherwise.
    ///
    /// Finally, all event weights are rescaled uniformly to preserve
    /// the total sun of weights.
    fn unweight(&mut self, events: &mut [Event]) -> Result<(), Self::Error> {
        let min_wt = self.min_wt;
        let nmin_wt = n64(min_wt);

        if min_wt == 0. || events.is_empty() {
            return Ok(());
        }
        let orig_wt_sum: N64 = events.par_iter().map(|e| e.weight()).sum();

        let distr = Uniform::try_from(0.0..min_wt).unwrap();
        for event in events.iter_mut() {
            let wt: f64 = event.weight().into();
            let awt = wt.abs();
            if awt > min_wt || awt == 0. {
                continue;
            }
            if distr.sample(&mut self.rng) < awt {
                event.rescale_weights(nmin_wt / awt);
            } else {
                event.rescale_weights(n64(0.));
            }
        }

        // rescale to ensure that the sum of weights is preserved exactly
        let final_wt_sum: N64 = events.par_iter().map(|e| e.weight()).sum();
        if final_wt_sum == 0. {
            warn!("Sum of weights is 0 after unweighting")
        } else {
            let reweight = orig_wt_sum / final_wt_sum;
            events
                .par_iter_mut()
                .for_each(|e| e.rescale_weights(reweight));
        }
        Ok(())
    }
}

/// Disable unweighting
pub struct NoUnweighter {}
impl Unweight for NoUnweighter {
    type Error = std::convert::Infallible;

    fn unweight(&mut self, _events: &mut [Event]) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Disable unweighting
pub const NO_UNWEIGHTING: NoUnweighter = NoUnweighter {};
