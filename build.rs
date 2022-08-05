use std::default::Default;
use std::env;
use std::path::PathBuf;

use anyhow::Result;
use cbindgen::Language;
use vergen::{vergen, Config, ShaKind};

fn main() -> Result<()> {
    let mut cfg = Config::default();
    *cfg.git_mut().sha_kind_mut() = ShaKind::Short;
    // If this is not run inside a git repository we get an error.
    // This happens when installing the crate via cargo.
    // As a quick fix, we just ignore it.
    let _ = vergen(cfg);

    if cfg!(target_family = "unix") {
        write_c_header()
    }

    #[cfg(feature = "ntuple")]
    compile_ntuple_reader()?;

    Ok(())
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

#[cfg(feature = "ntuple")]
fn compile_ntuple_reader() -> Result<()> {
    let bindings = bindgen::Builder::default()
        .header("ntuplereader/cnTupleReader.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .blocklist_item("true_")
        .blocklist_item("false_")
        .blocklist_item("__bool_true_false_are_defined")
        .newtype_enum("ColliderType")
        .generate()
        .expect("Failed to generate ntuple reader bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ntuplereader.rs"))
        .expect("Failed to write ntuple reader bindings!");

    println!("cargo:rerun-if-changed=ntuplereader/cnTupleReader.h");
    println!("cargo:rerun-if-changed=ntuplereader/cnTupleReader.cc");
    let mut cc_cmd = cc::Build::new();
    cc_cmd
        .cpp(true)
        .file("ntuplereader/cnTupleReader.cc");

    for flag in get_ntuplereader_flags("--cxxflags")? {
        cc_cmd.flag(&flag);
    }

    cc_cmd.compile("cntuplereader");

    let linker_flags = get_ntuplereader_flags("--rpath")?.into_iter().chain(
        get_ntuplereader_flags("--ldflags")?.into_iter()
    ).chain(
        get_ntuplereader_flags("--libs")?.into_iter()
    );
    for flag in linker_flags {
        println!("cargo:rustc-link-arg={flag}");
    }
    Ok(())
}

#[cfg(feature = "ntuple")]
fn get_ntuplereader_flags(flags: &str) -> Result<Vec<String>> {
    use std::{process::Command, str::from_utf8};
    use anyhow::{anyhow, Context};

    const CFG_CMD: &str = "nTupleReader-config";

    let cmd = format!("{CFG_CMD} {flags}");
    let output = Command::new(CFG_CMD).arg(flags).output().with_context(
        || format!("Failed to run `{cmd}`")
    )?;
    if !output.status.success() {
        if output.stderr.is_empty() {
            return Err(
                anyhow!("{CFG_CMD} {flags} failed without error messages")
            );
        } else {
            return Err(anyhow!(
                "{CFG_CMD} {flags} failed: {}",
                from_utf8(&output.stderr).unwrap()
            ));
        }
    }
    let args = from_utf8(&output.stdout).with_context(
        || format!("Failed to convert `{cmd}` output to utf8")
    )?;
    Ok(args.split_whitespace().map(|arg| arg.to_owned()).collect())
}
