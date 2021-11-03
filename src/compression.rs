use std::io::Write;

use bzip2::write::BzEncoder;
use flate2::write::GzEncoder;

#[derive(Debug, Copy, Clone)]
pub enum Compression {
    Bzip2,
    Gzip(u8),
    Lz4(u8),
    Zstd(u8),
}

pub fn compress_writer<'a, W: 'a + Write>(
        writer: W,
        compression: Option<Compression>,
) -> Result<Box<dyn Write + 'a>, std::io::Error> {
    match compression {
        Some(Compression::Bzip2) => {
            let encoder = BzEncoder::new(writer, bzip2::Compression::best());
            Ok(Box::new(encoder))
        },
        Some(Compression::Gzip(lvl)) => {
            let encoder = GzEncoder::new(writer, flate2::Compression::new(lvl.into()));
            Ok(Box::new(encoder))
        },
        Some(Compression::Lz4(lvl)) => {
            let encoder = lz4::EncoderBuilder::new().auto_flush(true).level(lvl.into()).build(writer)?;
            Ok(Box::new(encoder))
        },
        Some(Compression::Zstd(lvl)) => {
            let encoder = zstd::Encoder::new(writer, lvl.into())?;
            Ok(Box::new(encoder.auto_finish()))
        },
        None => Ok(Box::new(writer)),
    }
}
