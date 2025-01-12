mod opt_classify;
mod opt_common;
mod opt_cres;
mod opt_partition;

use crate::opt_cres::Opt;

use std::{
    env::var_os,
    ffi::OsStr,
    fs::{create_dir_all, File},
    io::{stdout, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use clap::{CommandFactory, Parser, ValueEnum};
use clap_complete::{generate, shells::*, Generator};
use dirs::home_dir;
use strum::{Display, EnumString};
use sysinfo::{get_current_pid, ProcessRefreshKind, RefreshKind, System};

#[derive(
    Copy,
    Clone,
    Debug,
    Display,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    EnumString,
    ValueEnum,
)]
#[strum(ascii_case_insensitive)]
#[strum(serialize_all = "lowercase")]
enum Shell {
    Bash,
    Elvish,
    Fish,
    #[allow(clippy::enum_variant_names)]
    PowerShell,
    Zsh,
}

#[derive(Debug, Parser)]
struct ShellSelect {
    /// Shell for which to generate completions
    ///
    /// If omitted, try to generate completions for the current shell
    #[clap(value_enum)]
    shell: Option<Shell>,
}

fn gen_completion<S: Copy + Generator, W: Write>(shell: S, mut to: W) {
    generate(shell, &mut Opt::command(), "cres", &mut to);
    generate(
        shell,
        &mut crate::opt_partition::Opt::command(),
        "cres-partition",
        &mut to,
    );
    generate(
        shell,
        &mut crate::opt_classify::Opt::command(),
        "cres-classify-events",
        &mut to,
    )
}

fn main() -> Result<()> {
    let shell = ShellSelect::parse().shell
        .map_or_else(get_parent_shell, Ok)
        .context("Failed to determine shell")?;
    eprintln!("Generating {shell} completions");
    match shell {
        Shell::Bash => gen_completion(Bash, gen_bash_outfile()?),
        Shell::Elvish => gen_completion(Elvish, &mut stdout()),
        Shell::Fish => gen_completion(Fish, gen_fish_outfile()?),
        Shell::PowerShell => gen_completion(PowerShell, &mut stdout()),
        Shell::Zsh => gen_completion(Zsh, &mut stdout()),
    }
    Ok(())
}

fn get_parent_shell() -> Result<Shell> {
    let system = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new()),
    );
    let my_pid = get_current_pid()
        .map_err(|err| anyhow!("{err}"))
        .context("Failed to get PID of the current process")?;
    let Some(process) = system.process(my_pid) else {
        bail!("Failed to access current process (PID {my_pid}")
    };
    let Some(parent_pid) = process.parent() else {
        bail!("Failed to get parent process PID for current process (PID {my_pid}")
    };
    let Some(parent_process) = system.process(parent_pid) else {
        bail!("Failed to access parent process (PID {my_pid}")
    };
    let Some(shell_name) = parent_process.name().to_str() else {
        bail!("Parent process name is not a valid UTF-8 string")
    };
    let shell = shell_name.try_into()
        .with_context(|| format!("{shell_name} is not a supported shell"))?;
    Ok(shell)
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
