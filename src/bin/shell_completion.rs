mod opt;
use crate::opt::Opt;

use std::{
    env::var_os,
    ffi::OsStr,
    fs::{create_dir_all, File},
    io::{stdout, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, shells::*, Generator};
use dirs::home_dir;
use strum::EnumString;

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    EnumString,
    ValueEnum,
)]
enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
}

#[derive(Debug, Parser)]
struct ShellSelect {
    /// Shell for which to generate completions
    #[clap(value_enum)]
    shell: Shell,
}

fn gen_completion<S: Generator, W: Write>(shell: S, mut to: W) {
    generate(shell, &mut Opt::command(), "cres", &mut to)
}

fn main() -> Result<()> {
    let shell = ShellSelect::parse().shell;
    match shell {
        Shell::Bash => gen_completion(Bash, gen_bash_outfile()?),
        Shell::Elvish => gen_completion(Elvish, &mut stdout()),
        Shell::Fish => gen_completion(Fish, gen_fish_outfile()?),
        Shell::PowerShell => gen_completion(PowerShell, &mut stdout()),
        Shell::Zsh => gen_completion(Zsh, &mut stdout()),
    }
    Ok(())
}

fn gen_bash_outfile() -> Result<File> {
    let mut outfile = var_os("$BASH_COMPLETION_USER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if let Some(dir) = var_os("XDG_DATA_HOME") {
                PathBuf::from_iter([
                    dir.as_os_str(),
                    OsStr::new("bash-completion"),
                ])
            } else {
                let mut dir = home_dir().expect("No home directory found");
                for part in
                    [".local", "share", "bash-completion", "completions"]
                {
                    dir.push(part);
                }
                dir
            }
        });
    outfile.push("cres.bash");
    create_file(outfile)
}

fn gen_fish_outfile() -> Result<File> {
    let mut outfile = var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let mut dir = home_dir().expect("No home directory found");
            for part in [".local", "share"] {
                dir.push(part);
            }
            dir
        });
    for part in ["fish", "vendor_completions.d", "cres,fish"] {
        outfile.push(part);
    }
    create_file(outfile)
}

fn create_file<P: AsRef<Path>>(name: P) -> Result<File> {
    create_dir_all(name.as_ref().parent().unwrap())?;
    File::create(name.as_ref())
        .with_context(|| format!("Failed to create {:?}", name.as_ref()))
}
