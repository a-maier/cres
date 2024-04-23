mod opt_classify;
mod opt_common;

use std::{
    fs::{create_dir_all, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use cres::{
    compression::{compress_writer, Compression},
    distance::EuclWithScaledPt,
    event::Event,
    formats::FileFormat,
    io::{detect_event_file_format, EventFileReader, FileReader},
    partition::VPTreePartition,
    prelude::{Converter, DefaultClustering},
    progress_bar::ProgressBar,
    traits::{Clustering, Progress, TryConvert},
    GIT_BRANCH, GIT_REV, VERSION,
};
use env_logger::Env;
use log::{debug, error, info, trace};

use crate::opt_classify::Opt;

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

    let part_file = opt.partition;
    let part_info = File::open(&part_file)
        .with_context(|| format!("Failed to open {part_file:?}"))?;
    type PartInfo =
        (DefaultClustering, VPTreePartition<Event, EuclWithScaledPt>);
    let (clustering, partitions): PartInfo = serde_yaml::from_reader(part_info)
        .with_context(|| {
            format!("Failed to read partition information from {part_file:?}")
        })?;
    info!("Using partitioning into {} regions", partitions.len());

    create_dir_all(&opt.outdir)
        .with_context(|| format!("Failed to create {:?}", opt.outdir))?;

    let classifier = Classifier {
        partitions,
        outdir: opt.outdir,
        compression: opt.compression,
        converter: Converter::new(),
        clustering,
    };

    //TODO: code duplication with Cres
    for file in opt.infiles {
        info!("Reading {file:?}");
        let format = detect_event_file_format(&file).with_context(|| {
            format!("Failed to detect event format for {file:?}")
        })?;
        match format {
            FileFormat::HepMC2 => {
                classifier.classify_byte_records(file, format)?
            }
            #[cfg(feature = "lhef")]
            FileFormat::Lhef => {
                classifier.classify_byte_records(file, format)?
            }
            #[cfg(feature = "stripper-xml")]
            FileFormat::StripperXml => {
                classifier.classify_byte_records(file, format)?
            }
            #[cfg(feature = "ntuple")]
            FileFormat::BlackHatNtuple => {
                classifier.classify_ntuple_records(file)?
            }
        }
    }
    info!("done");
    Ok(())
}

struct Classifier {
    partitions: VPTreePartition<Event, EuclWithScaledPt>,
    outdir: PathBuf,
    compression: Option<Compression>,
    converter: Converter,
    clustering: DefaultClustering,
}

impl Classifier {
    fn classify_byte_records(
        &self,
        file: PathBuf,
        format: FileFormat,
    ) -> Result<()> {
        let Classifier {
            partitions,
            outdir,
            compression,
            converter,
            clustering,
        } = self;
        let reader = FileReader::try_new(file.clone())
            .with_context(|| "Failed to read from {file:?}")?;
        let mut writers =
            make_byte_writers(partitions.len(), outdir, &file, *compression)
                .with_context(|| {
                    format!("Failed to create writers for {file:?}")
                })?;
        for writer in &mut writers {
            writer.write_all(reader.header())?;
        }
        let expected_nevents = reader.size_hint().0;
        let event_progress = if expected_nevents > 0 {
            ProgressBar::new(expected_nevents as u64, "events read")
        } else {
            ProgressBar::default()
        };
        for event in reader {
            let event = event?;
            let cres_event = converter.try_convert(event.clone())?;
            let cres_event = clustering.cluster(cres_event)?;
            let region = partitions.region(&cres_event);
            trace!("{event:#?} is in region {region}");
            let event = String::try_from(event).unwrap();
            let event = &trim_footer(&event, format);
            writers[region].write_all(event.as_bytes())?;
            event_progress.inc(1);
        }
        for writer in writers {
            if let Err(err) = write_footer(writer, format) {
                error!("Failed to write event file footer: {err}");
            }
        }
        Ok(())
    }

    #[cfg(feature = "ntuple")]
    fn classify_ntuple_records(&self, file: PathBuf) -> Result<()> {
        use cres::{io::EventRecord, ntuple::FileReader};
        use log::warn;
        use ntuple::Writer;

        let Classifier {
            partitions,
            outdir,
            compression,
            converter,
            clustering,
        } = self;
        if let Some(compression) = compression {
            warn!("Ignoring {compression:?} compression")
        }
        let reader = FileReader::try_new(file.clone())
            .with_context(|| "Failed to read from {file:?}")?;

        // TODO: code duplication with `make_byte_writers`
        let file = file.to_str().unwrap();
        let (prefix, suffix) = file.split_once('.').unwrap_or((file, ""));

        let make_outname = |i| {
            PathBuf::from_iter([
                outdir,
                PathBuf::from(format!("{prefix}.{i}.{suffix}")).as_path(),
            ])
        };

        let writers: Option<Vec<_>> = (0..partitions.len())
            .map(|i| Writer::new(make_outname(i), ""))
            .collect();
        let mut writers = writers
            .ok_or_else(|| anyhow!("Failed to create ntuple writers"))?;

        let expected_nevents = reader.size_hint().0;
        let event_progress =
            ProgressBar::new(expected_nevents as u64, "events read");
        for event in reader {
            let event = event?;
            let cres_event = converter.try_convert(event.clone())?;
            let cres_event = clustering.cluster(cres_event)?;
            let region = partitions.region(&cres_event);
            trace!("{event:#?} is in region {region}");
            let EventRecord::NTuple(event) = event else {
                unreachable!("Event record is not a BlackHatNtuple")
            };
            writers[region].write(&event)?;
            event_progress.inc(1);
        }
        Ok(())
    }
}

fn write_footer(
    mut writer: impl Write,
    format: FileFormat,
) -> std::io::Result<()> {
    writer.write_all(get_footer(format).as_bytes())?;
    writer.write_all(b"\n")
}

fn trim_footer(event: &str, format: FileFormat) -> &str {
    let footer = get_footer(format);
    if let Some(event) = event.trim_end().strip_suffix(footer) {
        event
    } else {
        event
    }
}

fn get_footer(format: FileFormat) -> &'static str {
    match format {
        FileFormat::HepMC2 => "HepMC::IO_GenEvent-END_EVENT_LISTING",
        #[cfg(feature = "lhef")]
        FileFormat::Lhef => "</LesHouchesEvents>",
        #[cfg(feature = "stripper-xml")]
        FileFormat::StripperXml => "</Eventrecord>",
        #[cfg(feature = "ntuple")]
        FileFormat::BlackHatNtuple => "",
    }
}

fn make_byte_writers(
    n: usize,
    outdir: &Path,
    file: &Path,
    compression: Option<Compression>,
) -> Result<Vec<Box<dyn Write>>> {
    let filename = file
        .file_name()
        .ok_or_else(|| anyhow!("Failed to extract filename from {file:?}"))?;
    let file = filename
        .to_str()
        .ok_or_else(|| anyhow!("Failed to convert {filename:?} to a string"))?;
    let (prefix, suffix) = file.split_once('.').unwrap_or((file, ""));

    let make_outname = |i| {
        PathBuf::from_iter([
            outdir,
            PathBuf::from(format!("{prefix}.{i}.{suffix}")).as_path(),
        ])
    };

    let mut res = Vec::with_capacity(n);
    for i in 0..n {
        let outname = make_outname(i);
        debug!("Events in region {i} will be written to {outname:?}");
        let outfile = File::create(&outname)
            .with_context(|| format!("Failed to create {outname:?}"))?;
        let out = BufWriter::new(outfile);
        let out = compress_writer(out, compression)
            .with_context(|| format!("Failed to compress {outname:?}"))?;
        res.push(out)
    }
    Ok(res)
}
