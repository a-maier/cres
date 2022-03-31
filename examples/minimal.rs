// minimal example for cell resampling
// run with `cargo run --release --example minimal -- IN.hepmc OUT.hepmc`
// set the environment variable `RUST_LOG=info` for command-line output
use std::error::Error;

use cres::hepmc2::{Converter, Reader, WriterBuilder};
use cres::prelude::*;

use env_logger;

fn main() -> Result<(), Box<dyn Error>> {
    // initialise logging from the RUST_LOG environment variable
    env_logger::init();

    // access command line arguments, ignoring the program name
    let mut args = std::env::args().skip(1);
    let infile = args.next().unwrap();
    let outfile = args.next().unwrap();

    // How to read events
    let reader = Reader::from_filenames(vec![infile])?;

    // How to convert into internal event format
    // To perform jet clustering use `ClusteringConverter` instead
    let converter = Converter::new();

    // Resample with default settings
    let resampler = ResamplerBuilder::default().build();

    // Where to write the output
    let writer = WriterBuilder::default().to_filename(outfile)?.build()?;

    let mut cres = CresBuilder {
        reader,
        converter,
        resampler,
        unweighter: NO_UNWEIGHTING, // disable unweighting
        writer,
    }
    .build();
    // Run the resampler
    cres.run()?;
    Ok(())
}
