//! Silo removal logic with type-safe validation.
//!
//! This module provides the `RemovableSilo` pattern which ensures that silos
//! are validated before removal. Use `TryFrom<Silo>` to validate, or
//! `RemovableSilo::from_silo_unchecked` to skip validation (for --force).

use crate::git::{self, Verbosity};
use crate::process;
use crate::silo::Silo;
use std::fmt;

/// Reasons why a silo cannot be removed without --force.
#[derive(Debug, Clone)]
pub enum RemovalBlocker {
    /// Silo has uncommitted changes
    UncommittedChanges(git::UncommittedStats),
    /// Silo has active processes running
    ActiveProcesses(Vec<process::ProcessInfo>),
    /// Silo has commits not merged into main branch
    UnmergedCommits(u32),
}

impl fmt::Display for RemovalBlocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemovalBlocker::UncommittedChanges(stats) => {
                write!(
                    f,
                    "Uncommitted changes: {} staged, {} modified, {} untracked",
                    stats.staged, stats.modified, stats.untracked
                )
            }
            RemovalBlocker::ActiveProcesses(processes) => {
                if processes.is_empty() {
                    write!(f, "Active processes: 0")
                } else {
                    let details: Vec<String> = processes
                        .iter()
                        .map(|p| format!("{} ({})", p.pid, p.command))
                        .collect();
                    write!(
                        f,
                        "Active processes ({}): {}",
                        processes.len(),
                        details.join(", ")
                    )
                }
            }
            RemovalBlocker::UnmergedCommits(count) => {
                write!(
                    f,
                    "Unmerged commits: {} commit(s) not in main branch",
                    count
                )
            }
        }
    }
}

/// Error returned when trying to convert a non-removable silo.
#[derive(Debug)]
pub struct RemovalError {
    /// The silo that cannot be removed
    pub silo: Silo,
    /// The reasons why removal is blocked
    pub blockers: Vec<RemovalBlocker>,
}

impl fmt::Display for RemovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Silo '{}' cannot be removed:", self.silo.name)?;
        for blocker in &self.blockers {
            write!(f, "\n  - {}", blocker)?;
        }
        Ok(())
    }
}

impl std::error::Error for RemovalError {}

/// A silo that has been validated for removal.
///
/// Created via `TryFrom<Silo>` which validates the silo can be removed,
/// or `RemovableSilo::from_silo_unchecked` to skip validation (for --force).
pub struct RemovableSilo {
    silo: Silo,
    main_branch: String,
    /// Whether the branch would be deleted (if merged into main)
    would_delete_branch: bool,
}

impl TryFrom<Silo> for RemovableSilo {
    type Error = RemovalError;

    #[allow(clippy::result_large_err)]
    fn try_from(silo: Silo) -> Result<Self, Self::Error> {
        let mut blockers = Vec::new();

        // Check for uncommitted changes
        let uncommitted = git::get_uncommitted_stats(&silo.storage_path);
        if !uncommitted.is_clean() {
            blockers.push(RemovalBlocker::UncommittedChanges(uncommitted));
        }

        // Check for active processes
        let processes = process::list_active(&silo.storage_path);
        if !processes.is_empty() {
            blockers.push(RemovalBlocker::ActiveProcesses(processes));
        }

        // Check for unmerged commits (commits ahead of main)
        let main_branch = Self::get_main_branch(&silo);
        let branch_name = silo.branch_name();
        let (ahead, _behind) = git::get_ahead_behind(&silo.storage_path, branch_name, &main_branch);
        if ahead > 0 {
            blockers.push(RemovalBlocker::UnmergedCommits(ahead));
        }

        if !blockers.is_empty() {
            return Err(RemovalError { silo, blockers });
        }

        // Pre-compute removal state
        let would_delete_branch =
            git::is_branch_merged(&silo.main_worktree, branch_name, &main_branch);

        Ok(Self {
            silo,
            main_branch,
            would_delete_branch,
        })
    }
}

impl RemovableSilo {
    /// Create a RemovableSilo without validation (for --force flag).
    pub fn from_silo_unchecked(silo: Silo) -> Self {
        let main_branch = Self::get_main_branch(&silo);
        let branch_name = silo.branch_name();
        let would_delete_branch =
            git::is_branch_merged(&silo.main_worktree, branch_name, &main_branch);

        Self {
            silo,
            main_branch,
            would_delete_branch,
        }
    }

    fn get_main_branch(silo: &Silo) -> String {
        git::list_worktrees(&silo.main_worktree)
            .ok()
            .and_then(|wts| wts.into_iter().next())
            .and_then(|wt| wt.branch)
            .unwrap_or_else(|| "main".to_string())
    }

    /// Get the silo name for display.
    pub fn name(&self) -> &str {
        &self.silo.name
    }

    /// Get a reference to the underlying silo.
    pub fn silo(&self) -> &Silo {
        &self.silo
    }

    /// Print what would happen in a dry run.
    pub fn print_dry_run(&self) {
        println!("Would remove silo: {}", self.silo.name);
        println!("  Path: {}", self.silo.storage_path.display());

        let branch_name = self.silo.branch_name();
        if self.would_delete_branch {
            println!(
                "  Would delete branch '{}' (merged into {})",
                branch_name, self.main_branch
            );
        } else {
            println!("  Would preserve branch '{}' (not merged)", branch_name);
        }
    }

    /// Execute the removal.
    ///
    /// If `force` is true, removes even if there are uncommitted changes.
    /// If `quiet` is true, suppresses normal output (errors still shown).
    pub fn remove(self, force: bool, quiet: bool) -> Result<(), String> {
        let verbosity = if quiet {
            Verbosity::Quiet
        } else {
            Verbosity::Verbose
        };

        git::remove_worktree(
            &self.silo.storage_path,
            &self.silo.main_worktree,
            force,
            verbosity,
        )?;

        // Clean up process tracking
        if let Err(e) = process::cleanup_tracking(&self.silo.storage_path) {
            eprintln!("Warning: {}", e);
        }

        // Clean up branch if merged
        let branch_name = self.silo.branch_name();
        let was_merged = git::cleanup_branch(
            &self.silo.main_worktree,
            branch_name,
            &self.main_branch,
            verbosity,
        );

        // Only print "preserved" message - git already outputs deletion info
        if !quiet && !was_merged {
            println!("Preserved branch '{}' (not merged)", branch_name);
        }

        Ok(())
    }
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
    fn test_removal_blocker_display() {
        let blocker = RemovalBlocker::UncommittedChanges(git::UncommittedStats {
            staged: 1,
            modified: 2,
            untracked: 3,
        });
        let display = format!("{}", blocker);
        assert!(display.contains("staged"));
        assert!(display.contains("modified"));
        assert!(display.contains("untracked"));
    }

    #[test]
    fn test_unmerged_commits_blocker_display() {
        let blocker = RemovalBlocker::UnmergedCommits(3);
        let display = format!("{}", blocker);
        assert!(display.contains("Unmerged commits"));
        assert!(display.contains("3 commit(s)"));
        assert!(display.contains("not in main branch"));
    }

    #[test]
    fn test_removal_error_display() {
        let silo = make_test_silo("test-branch");
        let error = RemovalError {
            silo,
            blockers: vec![RemovalBlocker::ActiveProcesses(vec![])],
        };
        let display = format!("{}", error);
        assert!(display.contains("test-branch"));
        assert!(display.contains("cannot be removed"));
    }

    #[test]
    fn test_from_silo_unchecked_preserves_name() {
        let silo = make_test_silo("my-feature");
        let removable = RemovableSilo::from_silo_unchecked(silo);
        assert_eq!(removable.name(), "my-feature");
    }
}
