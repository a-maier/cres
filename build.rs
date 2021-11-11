use std::env;

use cbindgen::Language;
use vergen::{Config, ShaKind, vergen};

fn main() {
    let mut cfg = Config::default();
    *cfg.git_mut().sha_kind_mut() = ShaKind::Short;
    vergen(cfg).unwrap();

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut config = cbindgen::Config::default();
    config.cpp_compat = true;
    config.function.must_use = Some(
        "__attribute__((warn_unused_result))".to_string()
    );
    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_language(Language::C)
        .with_include_guard("CRES_H")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("cres.h");
}
