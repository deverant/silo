//! The `rebase` command: rebase a silo's commits on top of the main branch.

use crate::git;

use super::{resolve_dash, resolve_silo};

pub fn run(name: String, dry_run: bool, quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;
    let silo = resolve_silo(&name)?;

    // Get main branch from the silo's main worktree
    let worktrees = git::list_worktrees(&silo.main_worktree)?;
    let main_branch = worktrees
        .first()
        .and_then(|wt| wt.branch.as_deref())
        .ok_or("Could not determine main branch")?;

    if dry_run {
        println!("Would rebase '{}' onto '{}'", silo.name, main_branch);
        return Ok(());
    }

    if !quiet {
        println!("Rebasing '{}' onto '{}'...", silo.name, main_branch);
        git::rebase_onto_interactive(&silo.storage_path, main_branch)?;
        println!("Rebase complete.");
    } else {
        git::rebase_onto(&silo.storage_path, main_branch)?;
    }

    Ok(())
}
