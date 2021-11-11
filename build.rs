use vergen::{Config, ShaKind, vergen};

fn main() {

    let mut cfg = Config::default();
    *cfg.git_mut().sha_kind_mut() = ShaKind::Short;
    vergen(cfg).unwrap();
}
