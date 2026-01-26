use crate::config::Config;
use crate::git;
use crate::names;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Information about a silo (isolated git worktree)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Silo {
    /// Name derived from directory name in worktrees directory
    pub name: String,
    /// Current git branch (None if detached HEAD)
    pub branch: Option<String>,
    /// Path to the main worktree (the original repo)
    pub main_worktree: PathBuf,
    /// Storage path of this silo worktree
    pub storage_path: PathBuf,
    /// Repository name (from git remote or directory)
    pub repo_name: String,
}

impl Silo {
    /// Get the branch name, falling back to the silo name if detached.
    pub fn branch_name(&self) -> &str {
        self.branch.as_deref().unwrap_or(&self.name)
    }
}

/// Get the base directory for all silos
/// Uses ~/.config/silo.toml if present, otherwise defaults to ~/.local/var/silo/
pub fn get_silo_base_dir() -> Result<PathBuf, String> {
    let config = Config::load()?;
    config.get_worktree_dir()
}

/// Get the full path for a specific silo
/// Format: ~/.local/var/silo/{repo-name}-{hash}/{branch-name}
pub fn get_silo_path(repo_name: &str, repo_path: &Path, branch: &str) -> Result<PathBuf, String> {
    let base = get_silo_base_dir()?;
    Ok(names::silo_storage_path(
        &base, repo_name, repo_path, branch,
    ))
}

/// Check if a path is within the silo base directory
#[must_use]
pub fn is_silo_path(path: &Path) -> bool {
    if let Ok(base) = get_silo_base_dir() {
        // Canonicalize both paths to handle symlinks (e.g., /var -> /private/var on macOS)
        let canon_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let canon_base = base.canonicalize().unwrap_or(base);
        canon_path.starts_with(&canon_base)
    } else {
        false
    }
}

/// Collect all silos across all repositories
pub fn collect_all_silos() -> Result<Vec<Silo>, String> {
    let base_dir = get_silo_base_dir()?;

    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut silos = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();

    let entries = std::fs::read_dir(&base_dir)
        .map_err(|e| format!("Failed to read silo directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let repo_silo_dir = entry.path();

        if !repo_silo_dir.is_dir() {
            continue;
        }

        // Find the first valid silo to get the main worktree location
        // Skip hidden files (like .DS_Store) and non-directories
        let first_silo = std::fs::read_dir(&repo_silo_dir)
            .ok()
            .and_then(|entries| {
                entries.filter_map(|e| e.ok()).find(|entry| {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    !name_str.starts_with('.') && entry.path().is_dir()
                })
            })
            .map(|e| e.path());

        let main_worktree_path = first_silo
            .as_ref()
            .and_then(|silo| git::get_main_worktree_from_silo(silo));

        let Some(main_path) = main_worktree_path else {
            continue;
        };

        // Get repo name from the main worktree
        let repo_name = match main_path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => {
                warn!(
                    path = %main_path.display(),
                    "Skipping repo with non-UTF-8 directory name"
                );
                continue;
            }
        };

        // List worktrees and collect silos
        if let Ok(worktrees) = git::list_worktrees(&main_path) {
            for wt in worktrees.iter().skip(1) {
                // Skip main worktree
                if !is_silo_path(&wt.path) {
                    continue;
                }

                // Get silo name, skip if not valid UTF-8
                let Some(silo_name) = wt.name() else {
                    warn!(
                        path = %wt.path.display(),
                        "Skipping silo with non-UTF-8 directory name"
                    );
                    continue;
                };

                // Deduplicate: only add if we haven't seen this path before
                if seen_paths.insert(wt.path.clone()) {
                    silos.push(Silo {
                        name: silo_name.to_string(),
                        branch: wt.branch.clone(),
                        main_worktree: main_path.clone(),
                        storage_path: wt.path.clone(),
                        repo_name: repo_name.clone(),
                    });
                }
            }
        }
    }

    Ok(silos)
}

/// Collect all silos for a specific repository.
pub fn collect_silos_for_repo(repo_root: &Path) -> Result<Vec<Silo>, String> {
    let worktrees = git::list_worktrees(repo_root)?;

    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Repository path has non-UTF-8 directory name".to_string())?
        .to_string();

    let silos = worktrees
        .into_iter()
        .skip(1) // Skip main worktree
        .filter(|wt| is_silo_path(&wt.path))
        .filter_map(|wt| {
            let name = wt.name().map(|s| s.to_string())?;
            Some(Silo {
                name,
                branch: wt.branch,
                main_worktree: repo_root.to_path_buf(),
                storage_path: wt.path,
                repo_name: repo_name.clone(),
            })
        })
        .collect();

    Ok(silos)
}

/// Collect silos that can be pruned (have no uncommitted changes) for a specific repo.
/// Returns Silo for each clean silo.
pub fn collect_prunable_repo(repo_root: &Path) -> Result<Vec<Silo>, String> {
    let worktrees = git::list_worktrees(repo_root)?;
    let mut to_prune = Vec::new();

    // Get repo name from the root
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Repository path has non-UTF-8 directory name".to_string())?
        .to_string();

    for wt in worktrees.iter().skip(1) {
        if !is_silo_path(&wt.path) {
            continue;
        }

        // Skip silos with non-UTF-8 names
        let Some(silo_name) = wt.name() else {
            continue;
        };

        if git::is_worktree_clean(&wt.path) {
            to_prune.push(Silo {
                name: silo_name.to_string(),
                branch: wt.branch.clone(),
                main_worktree: repo_root.to_path_buf(),
                storage_path: wt.path.clone(),
                repo_name: repo_name.clone(),
            });
        }
    }

    Ok(to_prune)
}

/// Collect silos that can be pruned (have no uncommitted changes) across all repos.
/// Returns Silo for each clean silo.
pub fn collect_prunable_all() -> Result<Vec<Silo>, String> {
    let base_dir = get_silo_base_dir()?;

    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut to_prune = Vec::new();

    let entries = std::fs::read_dir(&base_dir)
        .map_err(|e| format!("Failed to read silo directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let repo_silo_dir = entry.path();

        if !repo_silo_dir.is_dir() {
            continue;
        }

        // Find the first valid silo to get the main worktree location
        // Skip hidden files (like .DS_Store) and non-directories
        let first_silo = std::fs::read_dir(&repo_silo_dir)
            .ok()
            .and_then(|entries| {
                entries.filter_map(|e| e.ok()).find(|entry| {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    !name_str.starts_with('.') && entry.path().is_dir()
                })
            })
            .map(|e| e.path());

        let main_worktree_path = first_silo
            .as_ref()
            .and_then(|silo| git::get_main_worktree_from_silo(silo));

        let Some(ref main_path) = main_worktree_path else {
            continue;
        };

        let Ok(worktrees) = git::list_worktrees(main_path) else {
            continue;
        };

        // Get repo name from the main worktree, skip if non-UTF-8
        let Some(repo_name) = main_path.file_name().and_then(|n| n.to_str()) else {
            warn!(
                path = %main_path.display(),
                "Skipping repo with non-UTF-8 directory name"
            );
            continue;
        };
        let repo_name = repo_name.to_string();

        for wt in worktrees.iter().skip(1) {
            if !is_silo_path(&wt.path) {
                continue;
            }

            // Skip silos with non-UTF-8 names
            let Some(silo_name) = wt.name() else {
                continue;
            };

            if git::is_worktree_clean(&wt.path) {
                to_prune.push(Silo {
                    name: silo_name.to_string(),
                    branch: wt.branch.clone(),
                    main_worktree: main_path.clone(),
                    storage_path: wt.path.clone(),
                    repo_name: repo_name.clone(),
                });
            }
        }
    }

    Ok(to_prune)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_silo(name: &str, branch: Option<&str>) -> Silo {
        Silo {
            name: name.to_string(),
            branch: branch.map(|s| s.to_string()),
            main_worktree: PathBuf::from("/test/repo"),
            storage_path: PathBuf::from(format!("/test/silos/{}", name)),
            repo_name: "repo".to_string(),
        }
    }

    #[test]
    fn test_silo_branch_name_with_branch() {
        let silo = make_silo("feature", Some("feature-branch"));
        assert_eq!(silo.branch_name(), "feature-branch");
    }

    #[test]
    fn test_silo_branch_name_detached() {
        let silo = make_silo("feature", None);
        // Falls back to silo name when detached
        assert_eq!(silo.branch_name(), "feature");
    }

    #[test]
    fn test_silo_equality() {
        let silo1 = make_silo("feature", Some("branch"));
        let silo2 = make_silo("feature", Some("branch"));
        assert_eq!(silo1, silo2);
    }

    #[test]
    fn test_silo_inequality_name() {
        let silo1 = make_silo("feature1", Some("branch"));
        let silo2 = make_silo("feature2", Some("branch"));
        assert_ne!(silo1, silo2);
    }

    #[test]
    fn test_silo_clone() {
        let silo = make_silo("feature", Some("branch"));
        let cloned = silo.clone();
        assert_eq!(silo, cloned);
    }

    #[test]
    fn test_silo_debug() {
        let silo = make_silo("feature", Some("branch"));
        let debug = format!("{:?}", silo);
        assert!(debug.contains("feature"));
        assert!(debug.contains("branch"));
    }
}
