use cres::event::Event;

use rand::{Rng, distributions::{Distribution, Uniform}};
use rayon::prelude::*;
use noisy_float::prelude::*;

pub fn unweight<R: Rng> (
    events: &mut Vec<Event>,
    min_wt: f64,
    mut rng: R,
) {
    if events.is_empty() { return; }
    let orig_wt_sum: N64 = events.par_iter().map(|e| e.weight).sum();

    let distr = Uniform::from(0.0..min_wt);
    let keep = |e: &Event| {
        let wt: f64 = e.weight.into();
        let awt = wt.abs();
        if awt > min_wt {
            true
        } else {
            distr.sample(&mut rng) < awt
        }
    };
    events.retain(keep);

    let nmin_wt = n64(min_wt);
    events.par_iter_mut().for_each(
        |e| {
            let wt: f64 = e.weight.into();
            let awt = wt.abs();
            if awt < min_wt {
                e.weight = if wt > 0. {
                    nmin_wt
                } else {
                    -nmin_wt
                }
            }
        }
    );

    // rescale to ensure that the sum of weights is preserved exactly
    let final_wt_sum: N64 = events.par_iter().map(|e| e.weight).sum();
    let reweight = orig_wt_sum / final_wt_sum;
    events.par_iter_mut().for_each(|e| e.weight *= reweight);
}
