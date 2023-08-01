use crate::event::Event;
use crate::traits::Unweight;

use noisy_float::prelude::*;
use rand::{
    distributions::{Distribution, Uniform},
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
    fn unweight(
        &mut self,
        mut events: Vec<Event>,
    ) -> Result<Vec<Event>, Self::Error> {
        let min_wt = self.min_wt;
        if min_wt == 0. || events.is_empty() {
            return Ok(events);
        }
        let orig_wt_sum: N64 = events.par_iter().map(|e| e.weight()).sum();

        let distr = Uniform::from(0.0..min_wt);
        let keep = |e: &Event| {
            let wt: f64 = e.weight().into();
            let awt = wt.abs();
            if awt > min_wt {
                true
            } else {
                distr.sample(&mut self.rng) < awt
            }
        };
        events.retain(keep);

        let nmin_wt = n64(min_wt);
        events.par_iter_mut().for_each(|e| {
            let wt: f64 = e.weight().into();
            let awt = wt.abs();
            if awt < min_wt {
                e.rescale_weights(nmin_wt / awt);
            }
        });

        // rescale to ensure that the sum of weights is preserved exactly
        let final_wt_sum: N64 = events.par_iter().map(|e| e.weight()).sum();
        let reweight = orig_wt_sum / final_wt_sum;
        events.par_iter_mut().for_each(|e| e.rescale_weights(reweight));
        Ok(events)
    }
}

/// Disable unweighting
pub struct NoUnweighter {}
impl Unweight for NoUnweighter {
    type Error = std::convert::Infallible;

    fn unweight(
        &mut self,
        events: Vec<Event>,
    ) -> Result<Vec<Event>, Self::Error> {
        Ok(events)
    }
}

/// Disable unweighting
pub const NO_UNWEIGHTING: NoUnweighter = NoUnweighter {};
