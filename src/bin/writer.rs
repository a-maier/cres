use std::io::Write;

use bzip2::write::BzEncoder;
use flate2::write::GzEncoder;

use crate::opt::Compression;

pub(crate) fn make_writer<'a, W: 'a + Write>(
        writer: W,
        compression: Option<Compression>,
) -> Result<Box<dyn Write + 'a>, std::io::Error> {
    match compression {
        Some(Compression::Bzip2) => {
            let encoder = BzEncoder::new(writer, bzip2::Compression::best());
            Ok(Box::new(encoder))
        },
        Some(Compression::Gzip) => {
            let encoder = GzEncoder::new(writer, flate2::Compression::default());
            Ok(Box::new(encoder))
        },
        Some(Compression::Lz4) => {
            let encoder = lz4::EncoderBuilder::new().auto_flush(true).level(0).build(writer)?;
            Ok(Box::new(encoder))
        },
        Some(Compression::Zstd) => {
            let encoder = zstd::Encoder::new(writer, 0)?;
            Ok(Box::new(encoder.auto_finish()))
        },
        None => Ok(Box::new(writer)),
    }
}
