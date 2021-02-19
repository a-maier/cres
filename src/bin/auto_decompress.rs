use std::io::{BufRead, BufReader};

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use log::debug;

const GZIP_MAGIC_BYTES: [u8; 2] = [0x1f, 0x8b];
const BZIP2_MAGIC_BYTES: [u8; 3] = [b'B', b'Z', b'h'];

pub fn auto_decompress<'a, B: 'a + BufRead>(mut r: B) -> Box<dyn BufRead + 'a> {
    let bytes = match r.fill_buf() {
        Ok(bytes) => bytes,
        Err(_) => return Box::new(r),
    };
    if bytes.len() <= 3 {
        debug!("No decompression");
        Box::new(r)
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
