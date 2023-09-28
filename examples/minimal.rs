// minimal example for cell resampling
// run with `cargo run --release --example minimal -- IN.hepmc OUT.hepmc`
// set the environment variable `RUST_LOG=info` for command-line output
use std::error::Error;

use cres::prelude::*;

use env_logger;

fn main() -> Result<(), Box<dyn Error>> {
    // initialise logging from the RUST_LOG environment variable
    env_logger::init();

    // access command line arguments, ignoring the program name
    let mut args = std::env::args().skip(1);
    let infile = args.next().unwrap().into();
    let outfile = args.next().unwrap().into();

    // How to access (read & write) events
    let event_storage = StorageBuilder::default();
    let event_storage = event_storage.build_from_files(infile, outfile)?;

    // How to convert into internal event format
    let converter = Converter::new();

    // Cluster outgoing particles into IRC safe objects
    // To actually perform clustering use `DefaultClustering` instead
    let clustering = NO_CLUSTERING;

    // Resample with default settings
    let resampler = ResamplerBuilder::default().build();

    let mut cres = CresBuilder {
        event_storage,
        converter,
        clustering,
        resampler,
        unweighter: NO_UNWEIGHTING, // disable unweighting
    }
    .build();

    // Run the resampler
    cres.run()?;
    Ok(())
}
