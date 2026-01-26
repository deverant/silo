//! The `reset` command: reset a silo to the main worktree's current commit.

use crate::git::{self, Verbosity};
use crate::prompt;
use crate::removal::RemovalBlocker;
use crate::silo::Silo;

use super::{resolve_dash, resolve_silo};

/// Check if a silo has uncommitted changes or unmerged commits.
/// Returns a list of blockers if the silo is dirty.
fn check_dirty(silo: &Silo) -> Vec<RemovalBlocker> {
    let mut blockers = Vec::new();

    // Check for uncommitted changes
    let uncommitted = git::get_uncommitted_stats(&silo.storage_path);
    if !uncommitted.is_clean() {
        blockers.push(RemovalBlocker::UncommittedChanges(uncommitted));
    }

    // Check for unmerged commits (commits ahead of main)
    let main_branch = get_main_branch(silo);
    let branch_name = silo.branch_name();
    let (ahead, _behind) = git::get_ahead_behind(&silo.storage_path, branch_name, &main_branch);
    if ahead > 0 {
        blockers.push(RemovalBlocker::UnmergedCommits(ahead));
    }

    blockers
}

fn get_main_branch(silo: &Silo) -> String {
    git::list_worktrees(&silo.main_worktree)
        .ok()
        .and_then(|wts| wts.into_iter().next())
        .and_then(|wt| wt.branch)
        .unwrap_or_else(|| "main".to_string())
}

pub fn run(name: String, dry_run: bool, force: bool, quiet: bool) -> Result<(), String> {
    let name = resolve_dash(&name)?;
    let silo = resolve_silo(&name)?;

    // Check if silo is dirty
    let blockers = check_dirty(&silo);

    if !blockers.is_empty() && !force {
        eprintln!("Silo '{}' has uncommitted work:", silo.name);
        for blocker in &blockers {
            eprintln!("  - {}", blocker);
        }

        if !prompt::confirm("Reset anyway? All changes will be lost.") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Get the current HEAD commit from the main worktree
    let main_commit = git::get_head_commit(&silo.main_worktree)
        .map_err(|e| format!("Failed to get main worktree commit: {}", e))?;

    if dry_run {
        println!(
            "Would reset silo '{}' to commit {}",
            silo.name,
            &main_commit[..12.min(main_commit.len())]
        );
        println!("  Path: {}", silo.storage_path.display());
        if !blockers.is_empty() {
            println!("  Would discard:");
            for blocker in &blockers {
                println!("    - {}", blocker);
            }
        }
        return Ok(());
    }

    // Perform the reset and clean
    let verbosity = if quiet {
        Verbosity::Quiet
    } else {
        Verbosity::Verbose
    };
    git::reset_hard(&silo.storage_path, &main_commit, verbosity)
        .map_err(|e| format!("Failed to reset silo: {}", e))?;
    git::clean(&silo.storage_path, verbosity)
        .map_err(|e| format!("Failed to clean silo: {}", e))?;

    if !quiet {
        println!(
            "Reset silo '{}' to commit {}",
            silo.name,
            &main_commit[..12.min(main_commit.len())]
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_test_silo(name: &str) -> Silo {
        Silo {
            name: name.to_string(),
            branch: Some(name.to_string()),
            main_worktree: PathBuf::from("/tmp/test-repo"),
            storage_path: PathBuf::from(format!("/tmp/test-silos/{}", name)),
            repo_name: "test-repo".to_string(),
        }
    }

    #[test]
    fn test_get_main_branch_fallback() {
        let silo = make_test_silo("test-branch");
        // With a non-existent path, should fall back to "main"
        let branch = get_main_branch(&silo);
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_check_dirty_empty_for_nonexistent_path() {
        let silo = make_test_silo("test-branch");
        // With non-existent paths, git commands fail gracefully
        // and return empty/zero stats, so no blockers
        let blockers = check_dirty(&silo);
        assert!(blockers.is_empty());
    }
}
