use std::default::Default;
use std::env;
use std::path::PathBuf;

use anyhow::Result;
use cbindgen::{FunctionConfig, Language};
use vergen_git2::{Emitter, Git2Builder};

fn main() -> Result<()> {
    // optionally emit git branch and hash, ignoring all errors
    let _ = emit_vergen();

    if cfg!(target_family = "unix") {
        write_c_header()
    }

    #[cfg(feature = "ntuple")]
    // work around https://github.com/rust-lang/cargo/issues/12326
    // once this is fixed, we can remove the code below and use
    // ```
    // for flag in ntuple::ROOT_LINKER_FLAGS {
    //     println!("cargo:rustc-link-arg={flag}");
    // }
    // ```
    for flag in get_root_flags("--libs")? {
        println!("cargo:rustc-link-arg={flag}");
    }

    Ok(())
}

fn emit_vergen() -> Result<()> {
    let git2 = Git2Builder::default().branch(true).sha(true).build()?;
    Emitter::default()
        .fail_on_error()
        .quiet()
        .add_instructions(&git2)?
        .emit()?;
    Ok(())
}

fn write_c_header() {
    let out: PathBuf = [env::var("OUT_DIR").unwrap().as_str(), "cres.h"]
        .iter()
        .collect();

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let config = cbindgen::Config {
        cpp_compat: true,
        function: FunctionConfig {
            must_use: Some("__attribute__((warn_unused_result))".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
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

#[cfg(feature = "ntuple")]
fn get_root_flags(flags: &str) -> Result<Vec<String>> {
    use anyhow::{bail, Context};
    use std::{process::Command, str::from_utf8};

    const CFG_CMD: &str = "root-config";

    let cmd = format!("{CFG_CMD} {flags}");
    let output = Command::new(CFG_CMD)
        .arg(flags)
        .output()
        .with_context(|| format!("Failed to run `{cmd}`"))?;
    if !output.status.success() {
        if output.stderr.is_empty() {
            bail!("{CFG_CMD} {flags} failed without error messages");
        } else {
            bail!(
                "{CFG_CMD} {flags} failed: {}",
                from_utf8(&output.stderr).unwrap()
            );
        }
    }
    let args = from_utf8(&output.stdout)
        .with_context(|| format!("Failed to convert `{cmd}` output to utf8"))?;
    Ok(args.split_whitespace().map(|arg| arg.to_owned()).collect())
}
