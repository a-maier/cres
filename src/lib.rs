pub mod cell;
pub mod distance;
pub mod event;
pub mod file;
pub mod four_vector;
pub mod cres;
pub mod traits;
pub mod auto_decompress;
pub mod resampler;
pub mod progress_bar;
pub mod cell_collector;
pub mod compression;
pub mod unweight;
pub mod prelude;
pub mod hepmc2;

use lazy_static::lazy_static;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
lazy_static!{
    pub static ref VERSION_MAJOR: u32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    pub static ref VERSION_MINOR: u32 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
    pub static ref VERSION_PATCH: u32 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();
}
pub const GIT_REV: &str = env!("VERGEN_GIT_SHA_SHORT");
pub const GIT_BRANCH: &str = env!("VERGEN_GIT_BRANCH");
