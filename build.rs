use std::default::Default;
use std::env;
use std::path::PathBuf;

use cbindgen::Language;
use vergen::{vergen, Config, ShaKind};

fn main() {
    let mut cfg = Config::default();
    *cfg.git_mut().sha_kind_mut() = ShaKind::Short;
    vergen(cfg).unwrap();

    if cfg!(target_family = "unix") {
        write_c_header()
    }
}

fn write_c_header() {
    let out: PathBuf = [
        env::var("OUT_DIR").unwrap().as_str(),
        "cres.h"
    ].iter().collect();

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut config = cbindgen::Config {
        cpp_compat: true,
        ..Default::default()
    };
    config.function.must_use =
        Some("__attribute__((warn_unused_result))".to_string());
    cbindgen::Builder::new()
        .with_config(config)
        .with_header(
            "/** C API for cres
 *
 * See `examples/cres.c` and `examples/user_distance.c` for for usage.
 * The main function is `cres_run`.
 *
 * Functions return an integer, with `0` indicating success and
 * everything else indicating an error. Errors can be accessed with
 * `cres_get_last_err` and `cres_print_last_err`.
 *
 * License: GPL 3.0 or later
 * Author: Andreas Maier <andreas.martin.maier@desy.de>
*/",
        )
        .with_crate(crate_dir)
        .with_language(Language::C)
        .with_include_guard("CRES_H")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out);
}
