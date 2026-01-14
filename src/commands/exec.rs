//! The `exec` command: run a command in a silo directory.

use crate::shell;

use super::{resolve_dash, resolve_silo, run_command_in_dir};

pub fn run(name: String, command: &[String], quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;
    let silo = resolve_silo(&name)?;

    // Track this silo as the last used
    shell::write_directive("last", &name);

    run_command_in_dir(command, &silo.storage_path)?;

    if !quiet {
        eprintln!("[silo: {}]", name);
    }
    Ok(())
}
