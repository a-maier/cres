// the corresponding built-in method is currently (rust 1.70.0) only
// available in unstable, so we have to implement it ourselves
pub(crate) fn trim_ascii_start(buf: &[u8]) -> &[u8] {
    if let Some(pos) = buf.iter().position(|b| !b.is_ascii_whitespace()) {
        &buf[pos..]
    } else {
        &[]
    }
}

pub(crate) fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
