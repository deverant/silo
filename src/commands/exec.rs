//! The `exec` command: run a command in a silo directory.

use crate::config::Config;
use crate::shell;

use super::{apply_extra_args, resolve_dash, resolve_silo, run_command_in_dir};

pub fn run(name: String, command: &[String], config: &Config, quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;
    let silo = resolve_silo(&name)?;

    // Track this silo as the last used
    shell::write_directive("last", &name);

    let command = apply_extra_args(command, config.command_extra_args());
    run_command_in_dir(&command, &silo.storage_path)?;

    if !quiet {
        eprintln!("[silo: {}]", name);
    }
    Ok(())
}
