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
//! - [reader] defines readers from one or more event files
//! - [writer] for writing events to a file
//! - [event] for the internal event format
//! - [distance] for user-defined distance functions
//! - [seeds] and [resampler] for the resampling
//!

/// Partition events by iterative bisection
pub mod bisect;
#[cfg(target_family = "unix")]
#[cfg(feature = "capi")]
pub mod c_api;
/// Definition of event cells
pub mod cell;
pub mod cell_collector;
/// Output compression
pub mod compression;
/// Conversion between input events and internal format
pub mod converter;
/// Jet clustering helpers
pub mod cluster;
pub mod cres;
/// Distance functions
pub mod distance;
/// Scattering event class
pub mod event;
/// Thin wrapper around [std::fs::File]
pub mod file;
/// Four-vector class
pub mod four_vector;
/// HepMC2 interface
pub mod hepmc2;
/// Nearest neighbour search algorithms
pub mod neighbour_search;
/// Most important exports
pub mod prelude;
/// Progress bar
pub mod progress_bar;
/// Event readers
pub mod reader;
/// Cell resampling
pub mod resampler;
/// Cell seed selection
pub mod seeds;
/// Common traits
pub mod traits;
/// Unweighting
pub mod unweight;
/// Event writer
pub mod writer;
/// LesHouches Event File interface
#[cfg(feature = "lhef")]
pub mod lhef;
/// ntuple interface
#[cfg(feature = "ntuple")]
pub mod ntuple;

mod vptree;

use lazy_static::lazy_static;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
lazy_static! {
    pub static ref VERSION_MAJOR: u32 =
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    pub static ref VERSION_MINOR: u32 =
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
    pub static ref VERSION_PATCH: u32 =
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
}
pub const GIT_REV: Option<&str> = option_env!("VERGEN_GIT_SHA");
pub const GIT_BRANCH: Option<&str> = option_env!("VERGEN_GIT_BRANCH");

pub const FEATURES: [&str; NFEATURES] = [
    #[cfg(feature = "lhef")]
    "lhef",
    #[cfg(feature = "multiweight")]
    "multiweight",
    #[cfg(feature = "ntuple")]
    "ntuple",
    #[cfg(feature = "capi")]
    "capi",
];

const NFEATURES: usize = {
    #[allow(unused_mut)]
    let mut nfeatures = 0;
    #[cfg(feature = "lhef")]
    { nfeatures += 1; }
    #[cfg(feature = "multiweight")]
    { nfeatures += 1; }
    #[cfg(feature = "ntuple")]
    { nfeatures += 1; }
    #[cfg(feature = "capi")]
    { nfeatures += 1; }
    nfeatures
};
