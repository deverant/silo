//! The `exec` command: run a command in a silo directory.

use crate::config::Config;
use crate::runner;
use crate::shell;

use super::{resolve_dash, resolve_silo};

pub fn run(name: String, command: &[String], config: &Config, quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;
    let silo = resolve_silo(&name)?;

    // Track this silo as the last used
    shell::write_directive("last", &name);

    runner::run_command(command, &silo.storage_path, config)?;

    if !quiet {
        eprintln!("[silo: {}]", name);
    }
    Ok(())
}
