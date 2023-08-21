use std::default::Default;
use std::env;
use std::path::PathBuf;

use anyhow::Result;
use cbindgen::Language;
use vergen::EmitBuilder;

fn main() -> Result<()> {
    // optionally emit git branch and hash
    let _ = EmitBuilder::builder()
        .git_branch()
        .git_sha(true)
        // don't emit on error
        // we ignore the "fail" part
        .fail_on_error()
        .quiet()
        .emit();

    if cfg!(target_family = "unix") {
        write_c_header()
    }

    #[cfg(feature = "ntuple")]
    for flag in ntuple::ROOT_LINKER_FLAGS {
        println!("cargo:rustc-link-arg={flag}");
    }

    Ok(())
}

fn write_c_header() {
    let out: PathBuf = [env::var("OUT_DIR").unwrap().as_str(), "cres.h"]
        .iter()
        .collect();

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
