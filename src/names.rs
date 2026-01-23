//! Name logic for silos and repositories.
//!
//! This module handles:
//! - Unique storage path generation (with hash suffix for collision avoidance)
//! - Minimal display name generation (shortest unique name)
//! - Name resolution from user input

use crate::silo::Silo;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Result of name resolution
#[derive(Debug, PartialEq, Eq)]
pub enum ResolveResult<'a> {
    Found(&'a Silo),
    NotFound,
    Ambiguous(Vec<&'a Silo>),
}

/// Generate a short hash from a path for unique directory naming.
/// Returns first 8 characters of SHA-256 hash.
#[must_use]
pub fn path_hash(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    let result = hasher.finalize();
    // Take first 4 bytes (8 hex chars)
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        result[0], result[1], result[2], result[3]
    )
}

/// Generate the storage directory name for a repository.
/// Format: `{repo_name}-{hash}` where hash is first 8 chars of SHA-256 of path.
pub fn repo_storage_name(repo_name: &str, repo_path: &Path) -> String {
    let hash = path_hash(repo_path);
    format!("{}-{}", repo_name, hash)
}

/// Generate full silo storage path.
/// Format: `{base_dir}/{repo_name}-{hash}/{branch}`
pub fn silo_storage_path(
    base_dir: &Path,
    repo_name: &str,
    repo_path: &Path,
    branch: &str,
) -> PathBuf {
    let storage_name = repo_storage_name(repo_name, repo_path);
    base_dir.join(storage_name).join(branch)
}

/// Extract path components from a path, bottom-up (child first).
/// For `/a/b/c/repo`, returns `["repo", "c", "b", "a"]`.
fn path_components(path: &Path) -> Vec<String> {
    let mut components = Vec::new();
    let mut current = path;
    while let Some(name) = current.file_name() {
        components.push(name.to_string_lossy().to_string());
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }
    components
}

/// Generate minimal display names for a set of silos.
/// Returns a vector of display names in the same order as input silos.
///
/// The algorithm:
/// 1. Start with just the silo name (directory name)
/// 2. If there are duplicates, add repo name: repo/name
/// 3. If still duplicates, add parent directories until unique
///
/// If `require_repo_prefix` is true, always include at least the repo name
/// (useful when displaying silos outside of any repo context).
pub fn generate_display_names(silos: &[Silo], require_repo_prefix: bool) -> Vec<String> {
    if silos.is_empty() {
        return Vec::new();
    }

    // Track how many path components each silo needs (0 = just name, 1 = repo/name, etc.)
    // Start at depth 1 if repo prefix is required
    let initial_depth = if require_repo_prefix { 1 } else { 0 };
    let mut depth: Vec<usize> = vec![initial_depth; silos.len()];

    // Pre-compute path components for each silo's repo
    let repo_components: Vec<Vec<String>> = silos
        .iter()
        .map(|s| path_components(&s.main_worktree))
        .collect();

    // Iteratively increase depth for duplicates
    loop {
        // Build current display names (using silo.name instead of branch)
        let names: Vec<String> = silos
            .iter()
            .enumerate()
            .map(|(i, silo)| build_display_name(&silo.name, &repo_components[i], depth[i]))
            .collect();

        // Find duplicates
        let mut name_counts: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, name) in names.iter().enumerate() {
            name_counts.entry(name.as_str()).or_default().push(i);
        }

        // Check if all unique
        let duplicates: Vec<Vec<usize>> = name_counts
            .into_values()
            .filter(|indices| indices.len() > 1)
            .collect();

        if duplicates.is_empty() {
            return names;
        }

        // Increase depth for all duplicates
        let mut made_progress = false;
        for group in duplicates {
            for &idx in &group {
                // Only increase if we have more components available
                if depth[idx] < repo_components[idx].len() {
                    depth[idx] += 1;
                    made_progress = true;
                }
            }
        }

        // If we couldn't make progress (ran out of components), return what we have
        if !made_progress {
            return silos
                .iter()
                .enumerate()
                .map(|(i, silo)| build_display_name(&silo.name, &repo_components[i], depth[i]))
                .collect();
        }
    }
}

/// Build a display name with the given depth of path components.
/// depth=0: just name
/// depth=1: repo/name
/// depth=2: parent/repo/name
fn build_display_name(name: &str, repo_components: &[String], depth: usize) -> String {
    if depth == 0 || repo_components.is_empty() {
        return name.to_string();
    }

    let num_components = depth.min(repo_components.len());
    let path_parts: Vec<&str> = repo_components[..num_components]
        .iter()
        .rev()
        .map(|s| s.as_str())
        .collect();

    format!("{}/{}", path_parts.join("/"), name)
}

/// Resolve a user-provided name to a silo.
/// If `current_repo` is Some, prioritize matches from that repo.
///
/// The name can be:
/// - Just a silo name: "feature"
/// - Repo + name: "repoA/feature"
/// - Parent + repo + name: "org/repoA/feature"
pub fn resolve_name<'a>(
    input: &str,
    silos: &'a [Silo],
    current_repo: Option<PathBuf>,
) -> ResolveResult<'a> {
    if silos.is_empty() {
        return ResolveResult::NotFound;
    }

    let parts: Vec<&str> = input.split('/').collect();
    let silo_name = parts.last().copied().unwrap_or(input);

    // If we're in a repo, first try to find a match there
    if let Some(ref repo_path) = current_repo {
        let repo_matches: Vec<&Silo> = silos
            .iter()
            .filter(|s| &s.main_worktree == repo_path && s.name == silo_name)
            .collect();

        if repo_matches.len() == 1 {
            return ResolveResult::Found(repo_matches[0]);
        }
    }

    // Generate display names for all silos (minimal names for resolution)
    let display_names = generate_display_names(silos, false);

    // Find silos whose display name matches the input (or ends with the input)
    let matches: Vec<&Silo> = silos
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let display = &display_names[*i];
            // Exact match or the display name ends with the provided parts
            display == input || matches_suffix(display, &parts)
        })
        .map(|(_, silo)| silo)
        .collect();

    match matches.len() {
        0 => ResolveResult::NotFound,
        1 => ResolveResult::Found(matches[0]),
        _ => ResolveResult::Ambiguous(matches),
    }
}

/// Check if display name matches the given parts as a suffix.
/// "org/repo/name" matches ["name"], ["repo", "name"], ["org", "repo", "name"]
fn matches_suffix(display: &str, parts: &[&str]) -> bool {
    let display_parts: Vec<&str> = display.split('/').collect();
    if parts.len() > display_parts.len() {
        return false;
    }
    let start = display_parts.len() - parts.len();
    display_parts[start..] == *parts
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a Silo for testing
    fn make_silo(repo_path: &str, repo_name: &str, silo_name: &str) -> Silo {
        Silo {
            name: silo_name.to_string(),
            branch: Some(silo_name.to_string()), // For tests, name == branch
            main_worktree: PathBuf::from(repo_path),
            storage_path: PathBuf::from(format!(
                "/silos/{}-{}/{}",
                repo_name,
                &path_hash(Path::new(repo_path))[..4],
                silo_name
            )),
            repo_name: repo_name.to_string(),
        }
    }

    #[test]
    fn test_path_hash_consistent() {
        let path = Path::new("/Users/me/projects/repo");
        let hash1 = path_hash(path);
        let hash2 = path_hash(path);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 8);
    }

    #[test]
    fn test_path_hash_different_for_different_paths() {
        let hash1 = path_hash(Path::new("/dir1/repo"));
        let hash2 = path_hash(Path::new("/dir2/repo"));
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_silo_storage_path() {
        let path = silo_storage_path(
            Path::new("/silos"),
            "repo",
            Path::new("/Users/me/repo"),
            "feature",
        );
        let path_str = path.to_string_lossy();
        assert!(
            path_str.starts_with("/silos/repo-"),
            "path was: {}",
            path_str
        );
        assert!(path_str.ends_with("/feature"), "path was: {}", path_str);
    }

    #[test]
    fn test_path_components() {
        let components = path_components(Path::new("/a/b/c/repo"));
        assert_eq!(components, vec!["repo", "c", "b", "a"]);
    }

    #[test]
    fn test_display_names_unique_branches() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature1"),
            make_silo("/projects/repoA", "repoA", "feature2"),
        ];
        let names = generate_display_names(&silos, false);
        assert_eq!(names, vec!["feature1", "feature2"]);
    }

    #[test]
    fn test_display_names_same_branch_different_repos() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature"),
            make_silo("/projects/repoB", "repoB", "feature"),
        ];
        let names = generate_display_names(&silos, false);
        assert_eq!(names, vec!["repoA/feature", "repoB/feature"]);
    }

    #[test]
    fn test_display_names_same_repo_name_different_paths() {
        let silos = vec![
            make_silo("/org1/repo", "repo", "feature"),
            make_silo("/org2/repo", "repo", "feature"),
        ];
        let names = generate_display_names(&silos, false);
        assert_eq!(names, vec!["org1/repo/feature", "org2/repo/feature"]);
    }

    #[test]
    fn test_display_names_mixed_uniqueness() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "main"),
            make_silo("/projects/repoA", "repoA", "feature"),
            make_silo("/projects/repoB", "repoB", "feature"),
        ];
        let names = generate_display_names(&silos, false);
        // "main" is unique, both "feature" need repo prefix
        assert_eq!(names, vec!["main", "repoA/feature", "repoB/feature"]);
    }

    #[test]
    fn test_display_names_require_repo_prefix() {
        // When require_repo_prefix is true, always include repo name
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature1"),
            make_silo("/projects/repoA", "repoA", "feature2"),
        ];
        let names = generate_display_names(&silos, true);
        assert_eq!(names, vec!["repoA/feature1", "repoA/feature2"]);
    }

    #[test]
    fn test_display_names_require_repo_prefix_with_duplicates() {
        // When require_repo_prefix is true and repo names conflict, add parent dir
        let silos = vec![
            make_silo("/org1/repo", "repo", "feature"),
            make_silo("/org2/repo", "repo", "feature"),
        ];
        let names = generate_display_names(&silos, true);
        assert_eq!(names, vec!["org1/repo/feature", "org2/repo/feature"]);
    }

    #[test]
    fn test_display_names_require_repo_prefix_mixed() {
        // Mixed: some unique at repo level, some need parent
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "main"),
            make_silo("/org1/repo", "repo", "feature"),
            make_silo("/org2/repo", "repo", "feature"),
        ];
        let names = generate_display_names(&silos, true);
        assert_eq!(
            names,
            vec!["repoA/main", "org1/repo/feature", "org2/repo/feature"]
        );
    }

    #[test]
    fn test_resolve_simple_name() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature1"),
            make_silo("/projects/repoA", "repoA", "feature2"),
        ];
        let result = resolve_name("feature1", &silos, None);
        assert!(matches!(result, ResolveResult::Found(s) if s.name == "feature1"));
    }

    #[test]
    fn test_resolve_current_repo_priority() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature"),
            make_silo("/projects/repoB", "repoB", "feature"),
        ];
        let current = PathBuf::from("/projects/repoA");
        let result = resolve_name("feature", &silos, Some(current));
        assert!(
            matches!(result, ResolveResult::Found(s) if s.main_worktree == PathBuf::from("/projects/repoA"))
        );
    }

    #[test]
    fn test_resolve_qualified_name() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature"),
            make_silo("/projects/repoB", "repoB", "feature"),
        ];
        let result = resolve_name("repoB/feature", &silos, None);
        assert!(
            matches!(result, ResolveResult::Found(s) if s.main_worktree == PathBuf::from("/projects/repoB"))
        );
    }

    #[test]
    fn test_resolve_ambiguous_without_current_repo() {
        let silos = vec![
            make_silo("/projects/repoA", "repoA", "feature"),
            make_silo("/projects/repoB", "repoB", "feature"),
        ];
        let result = resolve_name("feature", &silos, None);
        assert!(matches!(result, ResolveResult::Ambiguous(v) if v.len() == 2));
    }

    #[test]
    fn test_resolve_not_found() {
        let silos = vec![make_silo("/projects/repoA", "repoA", "feature")];
        let result = resolve_name("nonexistent", &silos, None);
        assert!(matches!(result, ResolveResult::NotFound));
    }

    #[test]
    fn test_resolve_deeply_qualified() {
        let silos = vec![
            make_silo("/org1/repo", "repo", "feature"),
            make_silo("/org2/repo", "repo", "feature"),
        ];
        let result = resolve_name("org2/repo/feature", &silos, None);
        assert!(
            matches!(result, ResolveResult::Found(s) if s.main_worktree == PathBuf::from("/org2/repo"))
        );
    }

    #[test]
    fn test_display_names_deep_path_difference() {
        // Paths differ at org level, 3 components up from repo
        let silos = vec![
            make_silo("/workspace/org1/system/repo", "repo", "feature"),
            make_silo("/workspace/org2/system/repo", "repo", "feature"),
        ];
        let names = generate_display_names(&silos, false);
        // Should use org1/system/repo and org2/system/repo to distinguish
        assert_eq!(
            names,
            vec!["org1/system/repo/feature", "org2/system/repo/feature"]
        );
    }

    #[test]
    fn test_display_names_very_deep_path_difference() {
        // Paths differ 5 levels up - verifies no arbitrary max on components
        let silos = vec![
            make_silo("/root/a/b/c/d/e/repo", "repo", "silo"),
            make_silo("/root/x/b/c/d/e/repo", "repo", "silo"),
        ];
        let names = generate_display_names(&silos, false);
        // Should include all components needed: a/b/c/d/e/repo vs x/b/c/d/e/repo
        assert_eq!(names, vec!["a/b/c/d/e/repo/silo", "x/b/c/d/e/repo/silo"]);
    }

    #[test]
    fn test_display_names_difference_at_root() {
        // Paths differ only at the very first component
        let silos = vec![
            make_silo("/alpha/shared/path/to/repo", "repo", "feature"),
            make_silo("/beta/shared/path/to/repo", "repo", "feature"),
        ];
        let names = generate_display_names(&silos, false);
        assert_eq!(
            names,
            vec![
                "alpha/shared/path/to/repo/feature",
                "beta/shared/path/to/repo/feature"
            ]
        );
    }

    #[test]
    fn test_display_names_partial_path_overlap() {
        // Three silos: two share more path components than the third
        let silos = vec![
            make_silo("/workspace/team1/project/repo", "repo", "main"),
            make_silo("/workspace/team1/other/repo", "repo", "main"),
            make_silo("/workspace/team2/project/repo", "repo", "main"),
        ];
        let names = generate_display_names(&silos, false);
        // Each silo gets the minimal name needed to be unique:
        // - First and third conflict on "project/repo/main", need team1/team2 prefix
        // - Second is unique at "other/repo/main" (no other "other" directory)
        assert_eq!(
            names,
            vec![
                "team1/project/repo/main",
                "other/repo/main",
                "team2/project/repo/main"
            ]
        );
    }
}
