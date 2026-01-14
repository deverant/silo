//! The `merge` command: merge a silo's branch into the main worktree's current branch.

use crate::git;
use crate::silo;

use super::{resolve_dash, resolve_silo};

pub fn run(name: String, dry_run: bool, quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;

    // Get current repo root and verify we're in the main worktree
    let repo_root = git::get_repo_root()?;
    let cwd =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    if silo::is_silo_path(&cwd) {
        return Err("Must be run from the main worktree, not from a silo.".to_string());
    }

    let silo = resolve_silo(&name)?;

    // Verify the silo belongs to the current repo
    if silo.main_worktree != repo_root {
        return Err(format!(
            "Silo '{}' belongs to a different repository.",
            silo.name
        ));
    }

    if dry_run {
        println!("Would merge '{}' into current branch", silo.name);
        return Ok(());
    }

    let branch_name = silo.branch_name();
    if !quiet {
        println!("Merging '{}'...", silo.name);
        git::merge_branch_interactive(&repo_root, branch_name)?;
        println!("Merge complete.");
    } else {
        git::merge_branch(&repo_root, branch_name)?;
    }

    Ok(())
}
