mod opt;

use std::{path::PathBuf, io::Write};

use crate::opt::{FileFormat, parse_compr};

use anyhow::{Result, Context};
use clap::Parser;
use cres::{compression::Compression, GIT_REV, GIT_BRANCH, VERSION, reader::CombinedReader, traits::{Distance, Rewind, Progress, TryConvert, WriteEvent}, resampler::log2, distance::EuclWithScaledPt, bisect::circle_partition_with_progress, progress_bar::ProgressBar, converter::ClusteringConverter};
use env_logger::Env;
use log::{info, debug, error, trace};
use opt::JetDefinition;
use noisy_float::prelude::*;

// TODO: code duplication with opt::Opt
#[derive(Debug, Parser)]
#[clap(about, author, version)]
struct Opt {
    /// Output file prefix.
    ///
    /// Output is written to prefixX.suffix, where X is a number and
    /// suffix is chosen depending on the output format.
    #[clap(long, short, value_parser)]
    outfile: PathBuf,

    /// Output format
    #[clap(value_enum, long, default_value_t)]
    outformat: FileFormat,

    #[clap(short = 'c', long, value_parser = parse_compr,
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
    #[clap(long, value_parser = parse_npartitions)]
    partitions: u32,

    /// Number of threads
    ///
    /// If set to 0, a default number of threads is chosen.
    /// This default can be set with the `RAYON_NUM_THREADS` environment
    /// variable.
    #[clap(short, long, default_value_t)]
    threads: usize,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    infiles: Vec<PathBuf>,

    /// Weight of transverse momentum when calculating particle momentum distances.
    #[clap(long, default_value = "0.")]
    ptweight: f64,
}

fn main() -> Result<()> {
    let args = argfile::expand_args_from(
        std::env::args_os(),
        argfile::parse_fromfile,
        argfile::PREFIX,
    ).with_context(|| "Failed to read argument file")?;
    let opt = Opt::parse_from(args);

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

    //TODO: code duplication with Cres
    let mut reader = CombinedReader::from_files(opt.infiles)?;
    let expected_nevents = reader.size_hint().0;
    let event_progress = if expected_nevents > 0 {
        ProgressBar::new(expected_nevents as u64, "events read")
    } else {
        ProgressBar::default()
    };
    let mut converter = ClusteringConverter::new(opt.jet_def.into());
    let events: Result<Result<Vec<_>, _>, _> = (&mut reader)
        .map(|ev| ev.map(|e| converter.try_convert(e)))
        .inspect(|_| event_progress.inc(1))
        .collect();
    event_progress.finish();
    let mut events = events??;
    for (id, event) in events.iter_mut().enumerate() {
        event.id = id;
    }
    let nevents = events.len();

    info!("Splitting {nevents} events into {} parts", opt.partitions);

    let depth = log2(opt.partitions);
    let distance = EuclWithScaledPt::new(n64(opt.ptweight));
    let parts = circle_partition_with_progress(
        &mut events,
        |e1, e2| distance.distance(e1, e2),
        depth
    );
    debug_assert_eq!(parts.len(), opt.partitions as usize);

    let mut partition = vec![0; nevents];
    for (npart, events) in parts.into_iter().enumerate() {
        trace!("In partition {npart}:");
        for ev in events {
            trace!("event {}", ev.id());
            partition[ev.id()] = npart;
        }
    }

    let extension = {
        let base = opt.outformat.to_string();
        match opt.compression {
            Some(Compression::Bzip2) => base + ".bz2",
            Some(Compression::Gzip(_)) => base + ".gz",
            Some(Compression::Lz4(_)) => base + ".lz4",
            Some(Compression::Zstd(_)) => base + ".zst",
            None => base
        }
    };
    info!(
        "Writing output to {outfile}0.{extension}...{outfile}{}.{extension}",
        opt.partitions - 1,
        outfile = opt.outfile.display()
    );

    let outfiles = (0..opt.partitions).map(|n| {
        let mut path = opt.outfile.clone();
        let mut filename = opt.outfile.file_name().unwrap_or_default().to_owned();
        filename.push(n.to_string());
        path.set_file_name(filename);
        path.set_extension(&extension);
        path
    });

    let mut writers: Writers = match opt.outformat {
        FileFormat::HepMC2 => {
            let writers: Result<Vec<_>, _> = outfiles.map(|f| {
                cres::hepmc2::Writer::try_new(&f, opt.compression)
            }).collect();
            Writers::HepMC(writers?)
        },
        #[cfg(feature = "lhef")]
        FileFormat::Lhef => {
            let writers: Result<Vec<_>, _> = outfiles.map(|f| {
                cres::lhef::Writer::try_new(&f, opt.compression)
            }).collect();
            Writers::Lhef(writers?)
        },
        #[cfg(feature = "ntuple")]
        FileFormat::Root => {
            let writers: Result<Vec<_>, _> = outfiles.map(|f| {
                cres::ntuple::Writer::try_new(&f, opt.compression)
            }).collect();
            Writers::NTuple(writers?)
        },
        #[cfg(feature = "stripper-xml")]
        FileFormat::StripperXml => {
            let writers: Result<Vec<_>, _> = outfiles.map(|f| {
                cres::stripper_xml::Writer::try_new(&f, opt.compression)
            }).collect();
            Writers::StripperXml(writers?)
        },
    };

    reader.rewind()?;
    for (n, event) in reader.enumerate() {
        let event = event?;
        writers.write(partition[n], event)?;
    }

    match writers {
        Writers::HepMC(writers) => {
            for writer in writers {
                if let Err(err) = writer.finish() {
                    error!("{err}")
                }
            }
        },
        #[cfg(feature = "lhef")]
        Writers::Lhef(writers) => {
            for writer in writers {
                if let Err(err) = writer.finish() {
                    error!("{err}")
                }
            }
        },
        #[cfg(feature = "stripper-xml")]
        Writers::StripperXml(writers) => {
            for writer in writers {
                if let Err(err) = writer.finish() {
                    error!("{err}")
                }
            }
        },
        #[cfg(feature = "ntuple")]
        _ => { }
    }

    Ok(())
}

enum Writers {
    HepMC(Vec<cres::hepmc2::Writer<Box<dyn Write>>>),
    #[cfg(feature = "lhef")]
    Lhef(Vec<cres::lhef::Writer<Box<dyn Write>>>),
    #[cfg(feature = "ntuple")]
    NTuple(Vec<cres::ntuple::Writer>),
    #[cfg(feature = "stripper-xml")]
    StripperXml(Vec<cres::stripper_xml::Writer<Box<dyn Write>>>),
}

impl Writers {
    fn write(&mut self, idx: usize, event: avery::Event) -> Result<()> {
        match self {
            Writers::HepMC(writers) =>
                writers[idx].write(event).map_err(|e| e.into()),
            #[cfg(feature = "lhef")]
            Writers::Lhef(writers) =>
                writers[idx].write(event).map_err(|e| e.into()),
            #[cfg(feature = "ntuple")]
            Writers::NTuple(writers) =>
                writers[idx].write(event).map_err(|e| e.into()),
            #[cfg(feature = "stripper-xml")]
            Writers::StripperXml(writers) =>
                writers[idx].write(event).map_err(|e| e.into()),
        }
    }
}

fn parse_npartitions(s: &str) -> Result<u32, String> {
    use std::str::FromStr;

    match u32::from_str(s) {
        Ok(n) => if n.is_power_of_two() {
            Ok(n)
        } else {
            Err("has to be a power of two".to_string())
        }
        Err(err) => Err(err.to_string())
    }
}
