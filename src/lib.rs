//! `cres` is a crate for cell resampling, introduced in
//!
//! Unbiased Elimination of Negative Weights in Monte Carlo Samples\
//! J. Andersen, A. Maier\
//! [arXiv:2109.07851](https://arxiv.org/abs/2109.07851)
//!
//! # How to use
//!
//! Probably the best way to get started is to look at the examples, starting with
//! `examples/minimal.rs`.
//!
//! ## Most relevant modules
//!
//! - [prelude] exports a list of the most relevant classes and objects
//! - [cres] contains the main class and list the steps that are performed
//! - [hepmc2] contains a reader, converter, and writer for the HepMC 2 format
//! - [event] for the internal event format
//! - [distance] for user-defined distance functions
//! - [seeds] and [resampler] for the resampling
//!

/// Automatic input decompression
pub mod auto_decompress;
#[cfg(target_family = "unix")]
pub mod c_api;
/// Definition of event cells
pub mod cell;
pub mod cell_collector;
/// Output compression
pub mod compression;
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
/// Most important exports
pub mod prelude;
/// Progress bar
pub mod progress_bar;
/// Cell resampling
pub mod resampler;
/// Cell seed selection
pub mod seeds;
/// Common traits
pub mod traits;
/// Unweighting
pub mod unweight;

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
pub const GIT_REV: &str = env!("VERGEN_GIT_SHA_SHORT");
pub const GIT_BRANCH: &str = env!("VERGEN_GIT_BRANCH");
