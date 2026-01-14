use crate::config::Config;
use crate::git;
use crate::names;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
        let repo_name = main_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // List worktrees and collect silos
        if let Ok(worktrees) = git::list_worktrees(&main_path) {
            for wt in worktrees.iter().skip(1) {
                // Skip main worktree
                if !is_silo_path(&wt.path) {
                    continue;
                }

                // Deduplicate: only add if we haven't seen this path before
                if seen_paths.insert(wt.path.clone()) {
                    silos.push(Silo {
                        name: wt.name().to_string(),
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
        .unwrap_or("unknown")
        .to_string();

    let silos = worktrees
        .into_iter()
        .skip(1) // Skip main worktree
        .filter(|wt| is_silo_path(&wt.path))
        .map(|wt| Silo {
            name: wt.name().to_string(),
            branch: wt.branch,
            main_worktree: repo_root.to_path_buf(),
            storage_path: wt.path,
            repo_name: repo_name.clone(),
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
        .unwrap_or("unknown")
        .to_string();

    for wt in worktrees.iter().skip(1) {
        if !is_silo_path(&wt.path) {
            continue;
        }

        if git::is_worktree_clean(&wt.path) {
            to_prune.push(Silo {
                name: wt.name().to_string(),
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

        if let Some(ref main_path) = main_worktree_path
            && let Ok(worktrees) = git::list_worktrees(main_path)
        {
            // Get repo name from the main worktree
            let repo_name = main_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            for wt in worktrees.iter().skip(1) {
                if !is_silo_path(&wt.path) {
                    continue;
                }

                if git::is_worktree_clean(&wt.path) {
                    to_prune.push(Silo {
                        name: wt.name().to_string(),
                        branch: wt.branch.clone(),
                        main_worktree: main_path.clone(),
                        storage_path: wt.path.clone(),
                        repo_name: repo_name.clone(),
                    });
                }
            }
        }
    }

    Ok(to_prune)
}
