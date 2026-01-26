//! The `rm` command: remove a silo.

use crate::prompt;
use crate::removal;

use super::{resolve_dash, resolve_silo};

pub fn run(name: String, dry_run: bool, force: bool, quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;
    let silo = resolve_silo(&name)?;

    // Try to create a RemovableSilo, or use unchecked if force
    let removable = if force {
        removal::RemovableSilo::from_silo_unchecked(silo)
    } else {
        match removal::RemovableSilo::try_from(silo) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                eprintln!("Use --force to remove anyway.");
                return Err("Silo removal blocked".to_string());
            }
        }
    };

    if dry_run {
        removable.print_dry_run();
        return Ok(());
    }

    // Prompt for confirmation (unless force)
    if !force {
        let msg = format!("Remove silo '{}'?", removable.name());
        if !prompt::confirm(&msg) {
            println!("Aborted.");
            return Ok(());
        }
    }

    let display = removable.name().to_string();
    removable.remove(force, quiet)?;
    if !quiet {
        println!("Removed silo: {}", display);
    }

    Ok(())
}
