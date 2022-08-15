mod opt;

use std::{path::PathBuf, io::Write};

use crate::opt::{FileFormat, parse_compr};

use anyhow::{Result, Context};
use clap::Parser;
use cres::{compression::{Compression, compress_writer}, GIT_REV, GIT_BRANCH, VERSION, reader::CombinedReader, hepmc2::ClusteringConverter, traits::{TryConvert, Distance, Rewind}, resampler::log2, distance::EuclWithScaledPt, bisect::circle_partition, file::File};
use env_logger::Env;
use log::{info, debug};
use opt::{JetDefinition, is_power_of_two};
use noisy_float::prelude::*;

#[derive(Debug, Parser)]
#[clap(about, author, version)]
struct Opt {
    /// Output file prefix.
    ///
    /// Output is written to prefixX.suffix, where X is a number and
    /// suffix is chosen depending on the output format.
    #[clap(long, short, parse(from_os_str))]
    outfile: PathBuf,

    /// Output format
    #[clap(long, default_value_t)]
    outformat: FileFormat,

    #[clap(short = 'c', long, parse(try_from_str = parse_compr),
                help = "Compress hepmc output files.
Possible settings are 'bzip2', 'gzip', 'zstd', 'lz4'
Compression levels can be set with algorithm_level e.g. 'zstd_5'.
Maximum levels are 'gzip_9', 'zstd_19', 'lz4_16'.")]
    compression: Option<Compression>,

    #[clap(flatten)]
    jet_def: JetDefinition,

    /// Verbosity level
    ///
    /// Possible values with increasing amount of output are
    /// 'off', 'error', 'warn', 'info', 'debug', 'trace'.
    #[clap(short, long, default_value = "Info")]
    loglevel: String,

    /// Number of partitions
    ///
    /// The input event sample is split into the given number of
    /// partitions, which has to be a power of two. Each partition is
    /// written to its own output file.
    #[clap(long, validator = is_power_of_two)]
    partitions: u32,

    /// Number of threads
    ///
    /// If set to 0, a default number of threads is chosen.
    /// This default can be set with the `RAYON_NUM_THREADS` environment
    /// variable.
    #[clap(short, long, default_value_t)]
    threads: usize,

    /// Input files
    #[clap(name = "INFILES", parse(from_os_str))]
    infiles: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let opt = Opt::parse();

    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);

    rayon::ThreadPoolBuilder::new()
        .num_threads(opt.threads)
        .build_global()?;

    if let (Some(rev), Some(branch)) = (GIT_REV, GIT_BRANCH) {
        info!("cres-partition {} rev {} ({})", VERSION, rev, branch);
    } else {
        info!("cres-partition {}", VERSION);
    }

    debug!("settings: {:#?}", opt);

    let mut reader = CombinedReader::from_files(opt.infiles)?;
    let mut converter = ClusteringConverter::new(opt.jet_def.into());
    let events: Result<Result<Vec<_>, _>, _> = (&mut reader)
        .map(|ev| ev.map(|e| converter.try_convert(e)))
        .collect();
    let mut events = events??;
    let nevents = events.len();

    info!("Splitting {nevents} events into {} parts", opt.partitions);

    let depth = log2(opt.partitions);
    let distance = EuclWithScaledPt::new(n64(0.));
    let parts = circle_partition(
        &mut events,
        |e1, e2| distance.distance(e1, e2),
        depth
    );
    debug_assert_eq!(parts.len(), opt.partitions as usize);

    let mut partition = vec![0; nevents];
    for (npart, events) in parts.into_iter().enumerate() {
        for ev in events {
            partition[ev.id()] = npart;
        }
    }

    let extension = match opt.outformat {
        FileFormat::HepMC2 => {
            let base = "hepmc2".to_string();
            match opt.compression {
                Some(Compression::Bzip2) => base + ".bz2",
                Some(Compression::Gzip(_)) => base + ".gz",
                Some(Compression::Lz4(_)) => base + ".lz4",
                Some(Compression::Zstd(_)) => base + ".zst",
                None => base
            }}
        ,
        #[cfg(feature = "ntuple")]
        FileFormat::Root => "root".to_string()
    };
    let outfiles = (0..opt.partitions).map(|n| {
        let mut path = opt.outfile.clone();
        let mut filename = opt.outfile.file_name().unwrap_or_default().to_owned();
        filename.push(n.to_string());
        path.set_file_name(filename);
        path.set_extension(&extension);
        path
    });
    let mut writers: Vec<Box<dyn WriteEvent>> = Vec::with_capacity(
        opt.partitions as usize
    );
    for outfile in outfiles {
        match opt.outformat {
            FileFormat::HepMC2 => {
                let file = File::create(&outfile).with_context(
                    || format!("Failed to open {outfile:?}")
                )?;
                let writer = compress_writer(file, opt.compression)?;
                let writer = hepmc2::Writer::new(writer)?;
                writers.push(Box::new(writer));
            },
            #[cfg(feature = "ntuple")]
            FileFormat::Root => {
                use anyhow::anyhow;
                let writer = ntuplewriter::NTupleWriter::new(
                    &outfile,
                    "cres ntuple"
                ).ok_or_else(
                    || anyhow!("Failed to construct ntuple writer for {outfile:?}")
                )?;
                writers.push(Box::new(writer));
            }
        }
    }

    reader.rewind()?;
    for (n, event) in reader.enumerate() {
        let event = event?;
        writers[partition[n]].write(&event)?;
    }

    Ok(())
}

trait WriteEvent {
    fn write(&mut self, ev: &hepmc2::Event) -> Result<()>;
}

impl<T: Write> WriteEvent for hepmc2::Writer<T> {
    fn write(&mut self, ev: &hepmc2::Event) -> Result<()> {
        self.write(&ev).map_err(|e| e.into())
    }
}

#[cfg(feature = "ntuple")]
impl WriteEvent for ntuplewriter::NTupleWriter {
    fn write(&mut self, ev: &hepmc2::Event) -> Result<()> {
        self.write(&ev.into()).map_err(|e| e.into())
    }
}
