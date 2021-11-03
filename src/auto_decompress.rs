use std::io::{BufRead, BufReader};

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use log::debug;

const GZIP_MAGIC_BYTES: [u8; 2] = [0x1f, 0x8b];
const BZIP2_MAGIC_BYTES: [u8; 3] = [b'B', b'Z', b'h'];
const ZSTD_MAGIC_BYTES: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const LZ4_MAGIC_BYTES: [u8; 4] = [0x04, 0x22, 0x4d, 0x18];

pub fn auto_decompress<'a, B: 'a + BufRead>(mut r: B) -> Box<dyn BufRead + 'a> {
    let bytes = match r.fill_buf() {
        Ok(bytes) => bytes,
        Err(_) => return Box::new(r),
    };
    if bytes.len() <= 4 {
        debug!("No decompression");
        Box::new(r)
    } else if bytes[..4] == LZ4_MAGIC_BYTES {
        debug!("Decompress as lz4");
        Box::new(BufReader::new(lz4::Decoder::new(r).unwrap()))
    } else if bytes[..4] == ZSTD_MAGIC_BYTES {
        debug!("Decompress as zstd");
        Box::new(BufReader::new(zstd::stream::Decoder::new(r).unwrap()))
    } else if bytes[..2] == GZIP_MAGIC_BYTES {
        debug!("Decompress as gzip");
        Box::new(BufReader::new(GzDecoder::new(r)))
    } else if bytes[..3] == BZIP2_MAGIC_BYTES {
        debug!("Decompress as bzip2");
        Box::new(BufReader::new(BzDecoder::new(r)))
    } else {
        debug!("No decompression");
        Box::new(r)
    }
}
