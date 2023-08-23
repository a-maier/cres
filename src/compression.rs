use std::io::Write;

use bzip2::write::BzEncoder;
use flate2::write::GzEncoder;

/// Compression format
#[derive(Debug, Copy, Clone)]
pub enum Compression {
    /// The bzip2 format
    Bzip2,
    /// The gzip format with compression level as associated value
    Gzip(u8),
    /// The lz4 format with compression level as associated value
    Lz4(u8),
    /// The zstd format with compression level as associated value
    Zstd(u8),
}

/// Convert into a writer that compresses to the given format
pub fn compress_writer<'a, W: 'a + Write>(
    writer: W,
    compression: Option<Compression>,
) -> Result<Box<dyn Write + 'a>, std::io::Error> {
    match compression {
        Some(Compression::Bzip2) => {
            let encoder = BzEncoder::new(writer, bzip2::Compression::best());
            Ok(Box::new(encoder))
        }
        Some(Compression::Gzip(lvl)) => {
            let encoder =
                GzEncoder::new(writer, flate2::Compression::new(lvl.into()));
            Ok(Box::new(encoder))
        }
        Some(Compression::Lz4(lvl)) => {
            let encoder = lz4::EncoderBuilder::new()
                .auto_flush(true)
                .level(lvl.into())
                .build(writer)?;
            Ok(Box::new(encoder))
        }
        Some(Compression::Zstd(lvl)) => {
            let encoder = zstd::Encoder::new(writer, lvl.into())?;
            Ok(Box::new(encoder.auto_finish()))
        }
        None => Ok(Box::new(writer)),
    }
}
