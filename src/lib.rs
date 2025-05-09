//! `cres` is a crate for cell resampling, introduced in
//!
//! Unbiased Elimination of Negative Weights in Monte Carlo Samples\
//! J. Andersen, A. Maier\
//! [arXiv:2109.07851](https://arxiv.org/abs/2109.07851)
//!
//! Efficient negative-weight elimination in large high-multiplicity Monte Carlo event samples\
//! Jeppe R. Andersen, Andreas Maier, Daniel Maître\
//! [arXiv:2303.15246](https://arxiv.org/abs/2303.15246)
//!
//! # How to use
//!
//! Probably the best way to get started is to look at the examples, starting with
//! `examples/minimal.rs`.
//!
//! ## Most relevant modules
//!
//! - [prelude] exports a list of the most relevant classes and objects
//! - [cres] contains the main class and lists the steps that are performed
//! - [io] event input and output (from and to files)
//! - [event] for the internal event format
//! - [distance] for user-defined distance functions
//! - [seeds] and [resampler] for the resampling
//!
#![warn(missing_docs)]

#[cfg(target_family = "unix")]
#[cfg(feature = "capi")]
pub mod c_api;
/// Definition of event cells
pub mod cell;
/// Callbacks used upon cell construction and when writing out events
pub mod cell_collector;
/// Jet clustering helpers
pub mod cluster;
/// Output compression
pub mod compression;
pub mod cres;
/// Distance functions
pub mod distance;
/// Scattering event class
pub mod event;
/// Supported event file formats
pub mod formats;
/// Four-vector class
pub mod four_vector;
/// HepMC2 interface
pub mod hepmc2;
/// Event input/output
pub mod io;
/// LesHouches Event File interface
#[cfg(feature = "lhef")]
pub mod lhef;
/// Nearest neighbour search algorithms
pub mod neighbour_search;
/// BlackHat ntuple interface
#[cfg(feature = "ntuple")]
pub mod ntuple;
/// Phase-space partitions
pub mod partition;
/// Most important exports
pub mod prelude;
/// Progress bar
pub mod progress_bar;
/// Cell resampling
pub mod resampler;
/// Cell seed selection
pub mod seeds;
/// STRIPPER XML interface
#[cfg(feature = "stripper-xml")]
pub mod stripper_xml;
/// Common traits
pub mod traits;
/// Unweighting
pub mod unweight;
/// Vantage-point tree
pub mod vptree;

mod parsing;
mod util;

const fn parse_version(s: &str) -> u32 {
    match u32::from_str_radix(s, 10) {
        Ok(int) => int,
        Err(_) => panic!("Failed to parse version number"),
    }
}

/// cres version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Major version number
pub const VERSION_MAJOR: u32 = parse_version(env!("CARGO_PKG_VERSION_MAJOR"));
/// Minor version number
pub const VERSION_MINOR: u32 = parse_version(env!("CARGO_PKG_VERSION_MINOR"));
/// Patch version number
pub const VERSION_PATCH: u32 = parse_version(env!("CARGO_PKG_VERSION_PATCH"));
/// Hash of the compiled git commit
pub const GIT_REV: Option<&str> = option_env!("VERGEN_GIT_SHA");
/// git branch during compilation
pub const GIT_BRANCH: Option<&str> = option_env!("VERGEN_GIT_BRANCH");

/// Features enabled during compilation
pub const FEATURES: [&str; NFEATURES] = [
    #[cfg(feature = "lhef")]
    "lhef",
    #[cfg(feature = "multiweight")]
    "multiweight",
    #[cfg(feature = "ntuple")]
    "ntuple",
    #[cfg(feature = "stripper-xml")]
    "stripper-xml",
    #[cfg(feature = "capi")]
    "capi",
];

const NFEATURES: usize = {
    #[allow(unused_mut)]
    let mut nfeatures = 0;
    #[cfg(feature = "lhef")]
    {
        nfeatures += 1;
    }
    #[cfg(feature = "multiweight")]
    {
        nfeatures += 1;
    }
    #[cfg(feature = "ntuple")]
    {
        nfeatures += 1;
    }
    #[cfg(feature = "stripper-xml")]
    {
        nfeatures += 1;
    }
    #[cfg(feature = "capi")]
    {
        nfeatures += 1;
    }
    nfeatures
};

pub use noisy_float::prelude::{n64, Float, N64};
pub use particle_id::*;
