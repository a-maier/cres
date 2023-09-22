mod opt_common;

use std::{io::Write, path::PathBuf, fs::File};

use crate::opt_common::{parse_compr, FileFormat};

use anyhow::{Context, Result};
use clap::Parser;
use cres::{GIT_REV, GIT_BRANCH, VERSION, reader::CombinedReader, progress_bar::ProgressBar, compression::Compression, prelude::ClusteringConverter, partition::VPTreePartition, event::Event, distance::EuclWithScaledPt, traits::{TryConvert, WriteEvent, Progress}};
use env_logger::Env;
use log::{debug, error, info, trace};

// TODO: code duplication with opt::Opt
#[derive(Debug, Parser)]
#[clap(about, author, version)]
struct Opt {
    /// File containing partitioning information
    ///
    /// This is a file created with `cres-make-partition`
    #[clap(long, short, value_parser)]
    partitioning: PathBuf,

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

    /// Verbosity level
    ///
    /// Possible values with increasing amount of output are
    /// 'off', 'error', 'warn', 'info', 'debug', 'trace'.
    #[clap(short, long, default_value = "Info")]
    loglevel: String,

    /// Input files
    #[clap(name = "INFILES", value_parser)]
    infiles: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = argfile::expand_args_from(
        std::env::args_os(),
        argfile::parse_fromfile,
        argfile::PREFIX,
    )
    .with_context(|| "Failed to read argument file")?;
    let opt = Opt::parse_from(args);

    let env = Env::default().filter_or("CRES_LOG", &opt.loglevel);
    env_logger::init_from_env(env);

    if let (Some(rev), Some(branch)) = (GIT_REV, GIT_BRANCH) {
        info!("cres-partition {VERSION} rev {rev} ({branch})");
    } else {
        info!("cres-partition {VERSION}");
    }

    debug!("settings: {:#?}", opt);

    let part_file = opt.partitioning;
    let part_info = File::open(&part_file).with_context(
        || format!("Failed to open {part_file:?}")
    )?;
    type PartInfo = (ClusteringConverter, VPTreePartition<Event, EuclWithScaledPt>);
    let (mut converter, partitions): PartInfo = serde_yaml::from_reader(part_info).with_context(
        || format!("Failed to read partition information from {part_file:?}")
    )?;

    let extension = {
        let base = opt.outformat.to_string();
        match opt.compression {
            Some(Compression::Bzip2) => base + ".bz2",
            Some(Compression::Gzip(_)) => base + ".gz",
            Some(Compression::Lz4(_)) => base + ".lz4",
            Some(Compression::Zstd(_)) => base + ".zst",
            None => base,
        }
    };
    info!(
        "Writing output to {outfile}0.{extension}...{outfile}{}.{extension}",
        partitions.len() - 1,
        outfile = opt.outfile.display()
    );

    let outfiles = (0..partitions.len()).map(|n| {
        let mut path = opt.outfile.clone();
        let mut filename =
            opt.outfile.file_name().unwrap_or_default().to_owned();
        filename.push(n.to_string());
        path.set_file_name(filename);
        path.set_extension(&extension);
        path
    });

    let mut writers: Writers = match opt.outformat {
        FileFormat::HepMC2 => {
            let writers: Result<Vec<_>, _> = outfiles
                .map(|f| cres::hepmc2::Writer::try_new(&f, opt.compression))
                .collect();
            Writers::HepMC(writers?)
        }
        #[cfg(feature = "lhef")]
        FileFormat::Lhef => {
            let writers: Result<Vec<_>, _> = outfiles
                .map(|f| cres::lhef::Writer::try_new(&f, opt.compression))
                .collect();
            Writers::Lhef(writers?)
        }
        #[cfg(feature = "ntuple")]
        FileFormat::Root => {
            let writers: Result<Vec<_>, _> = outfiles
                .map(|f| cres::ntuple::Writer::try_new(&f, opt.compression))
                .collect();
            Writers::NTuple(writers?)
        }
        #[cfg(feature = "stripper-xml")]
        FileFormat::StripperXml => {
            let writers: Result<Vec<_>, _> = outfiles
                .map(|f| {
                    cres::stripper_xml::Writer::try_new(&f, opt.compression)
                })
                .collect();
            Writers::StripperXml(writers?)
        }
    };

    //TODO: code duplication with Cres
    let reader = CombinedReader::from_files(opt.infiles)?;
    let expected_nevents = reader.size_hint().0;
    let event_progress = if expected_nevents > 0 {
        ProgressBar::new(expected_nevents as u64, "events read")
    } else {
        ProgressBar::default()
    };

    for event in reader {
        let event = event?;
        let cres_event = converter.try_convert(event.clone())?;
        let region = partitions.region(&cres_event);
        trace!("{event:#?} is in region {region}");
        writers.write(region, event)?;
        event_progress.inc(1);
    }

    match writers {
        Writers::HepMC(writers) => {
            for writer in writers {
                if let Err(err) = writer.finish() {
                    error!("{err}")
                }
            }
        }
        #[cfg(feature = "lhef")]
        Writers::Lhef(writers) => {
            for writer in writers {
                if let Err(err) = writer.finish() {
                    error!("{err}")
                }
            }
        }
        #[cfg(feature = "stripper-xml")]
        Writers::StripperXml(writers) => {
            for writer in writers {
                if let Err(err) = writer.finish() {
                    error!("{err}")
                }
            }
        }
        #[cfg(feature = "ntuple")]
        _ => {}
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
            Writers::HepMC(writers) => {
                writers[idx].write(event).map_err(|e| e.into())
            }
            #[cfg(feature = "lhef")]
            Writers::Lhef(writers) => {
                writers[idx].write(event).map_err(|e| e.into())
            }
            #[cfg(feature = "ntuple")]
            Writers::NTuple(writers) => {
                writers[idx].write(event).map_err(|e| e.into())
            }
            #[cfg(feature = "stripper-xml")]
            Writers::StripperXml(writers) => {
                writers[idx].write(event).map_err(|e| e.into())
            }
        }
    }
}
