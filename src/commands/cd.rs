//! The `cd` command: navigate to a silo directory.

use crate::git;
use crate::shell;

use super::{resolve_dash, resolve_silo};

pub fn run(name: Option<String>) -> Result<(), String> {
    // If no name provided, navigate to the main worktree
    let Some(name) = name else {
        return cd_to_main_worktree();
    };

    let name = resolve_dash(&name)?;

    // First, check if we're in a repo and the name matches the main branch
    if let Some(repo_root) = git::try_get_repo_root()
        && let Ok(worktrees) = git::list_worktrees(&repo_root)
        && let Some(main_wt) = worktrees.first()
        && main_wt.branch.as_deref() == Some(name.as_str())
    {
        // Navigate to the main worktree (don't track as "last" silo)
        shell::write_directive("cd", &main_wt.path.display().to_string());
        println!("{}", main_wt.path.display());
        return Ok(());
    }

    // Otherwise, resolve the silo name
    let silo = resolve_silo(&name)?;

    // Write directives for shell wrapper
    shell::write_directive("cd", &silo.storage_path.display().to_string());
    shell::write_directive("last", &name);

    // Also print path for non-shell-wrapper usage (cd $(silo cd branch))
    println!("{}", silo.storage_path.display());
    Ok(())
}

fn cd_to_main_worktree() -> Result<(), String> {
    let repo_root =
        git::try_get_repo_root().ok_or_else(|| "Not in a git repository".to_string())?;

    let worktrees = git::list_worktrees(&repo_root)?;
    let main_wt = worktrees
        .first()
        .ok_or_else(|| "No worktrees found".to_string())?;

    shell::write_directive("cd", &main_wt.path.display().to_string());
    println!("{}", main_wt.path.display());
    Ok(())
}
