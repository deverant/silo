use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
}

impl Worktree {
    /// Get the worktree name from its directory path.
    pub fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    /// Get the branch name, or "(detached)" if in detached HEAD state.
    pub fn branch_name(&self) -> &str {
        self.branch.as_deref().unwrap_or("(detached)")
    }
}

/// Information about a repository needed for naming
#[derive(Debug, Clone)]
pub struct RepoInfo {
    /// The path to the main worktree
    pub main_worktree: PathBuf,
    /// The repository name
    pub name: String,
}

/// Create a git command with working directory set
fn git_command(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root);
    cmd
}

/// Run a git command and return stdout on success, or formatted error on failure
fn run_git(cmd: &mut Command, error_context: &str) -> Result<String, String> {
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{}: {}", error_context, stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a git command and print its output (stdout and stderr).
/// Returns the output on success, or formatted error on failure.
fn run_git_verbose(cmd: &mut Command, error_context: &str) -> Result<String, String> {
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Print stdout if non-empty
    if !stdout.trim().is_empty() {
        print!("{}", stdout);
    }

    // Print stderr (git often writes progress to stderr even on success)
    if !stderr.trim().is_empty() && output.status.success() {
        eprint!("{}", stderr);
    }

    if !output.status.success() {
        return Err(format!("{}: {}", error_context, stderr.trim()));
    }

    Ok(stdout.into_owned())
}

/// Run a git command with inherited stdin/stdout/stderr for interactive use.
/// Returns Ok on success, or formatted error on failure.
fn run_git_interactive(cmd: &mut Command, error_context: &str) -> Result<(), String> {
    let status = cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !status.success() {
        return Err(error_context.to_string());
    }
    Ok(())
}

/// Get complete repository information (name and root path)
pub fn get_repo_info() -> Result<RepoInfo, String> {
    let main_worktree = get_repo_root()?;
    let name = get_repo_name()?;
    Ok(RepoInfo {
        main_worktree,
        name,
    })
}

/// Get the root directory of the current git repository
pub fn get_repo_root() -> Result<PathBuf, String> {
    try_get_repo_root().ok_or_else(|| "Not in a git repository".to_string())
}

/// Try to get the root directory of the current git repository
/// Returns None if not in a git repository
pub fn try_get_repo_root() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Some(PathBuf::from(path))
}

/// Get the repository name from the origin remote URL or directory name
pub fn get_repo_name() -> Result<String, String> {
    // Try to get from origin URL first
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Extract repo name from URL (handles both HTTPS and SSH)
        if let Some(name) = extract_repo_name_from_url(&url) {
            return Ok(name);
        }
    }

    // Fall back to directory name
    let root = get_repo_root()?;
    root.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Could not determine repository name".to_string())
}

fn extract_repo_name_from_url(url: &str) -> Option<String> {
    // Handle URLs like:
    // git@github.com:user/repo.git
    // https://github.com/user/repo.git
    // https://github.com/user/repo
    let name = url
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .or_else(|| url.rsplit(':').next())?;

    Some(name.to_string())
}

/// Create a new worktree with a new branch
pub fn create_worktree(path: &Path, branch: &str, repo_root: &Path) -> Result<(), String> {
    run_git(
        git_command(repo_root)
            .args(["worktree", "add", "-b", branch])
            .arg(path),
        "Failed to create worktree",
    )?;
    Ok(())
}

/// Create a new worktree with a new branch, printing git output
pub fn create_worktree_verbose(path: &Path, branch: &str, repo_root: &Path) -> Result<(), String> {
    run_git_verbose(
        git_command(repo_root)
            .args(["worktree", "add", "-b", branch])
            .arg(path),
        "Failed to create worktree",
    )?;
    Ok(())
}

/// Remove a worktree
/// If force is true, removes even if there are uncommitted changes
pub fn remove_worktree(path: &Path, repo_root: &Path, force: bool) -> Result<(), String> {
    let mut cmd = git_command(repo_root);
    cmd.args(["worktree", "remove"]);
    if force {
        cmd.arg("--force");
    }
    cmd.arg(path);
    run_git(&mut cmd, "Failed to remove worktree")?;
    Ok(())
}

/// Remove a worktree, printing git output
/// If force is true, removes even if there are uncommitted changes
pub fn remove_worktree_verbose(path: &Path, repo_root: &Path, force: bool) -> Result<(), String> {
    let mut cmd = git_command(repo_root);
    cmd.args(["worktree", "remove"]);
    if force {
        cmd.arg("--force");
    }
    cmd.arg(path);
    run_git_verbose(&mut cmd, "Failed to remove worktree")?;
    Ok(())
}

/// List all worktrees for the current repository
pub fn list_worktrees(repo_root: &Path) -> Result<Vec<Worktree>, String> {
    let output = run_git(
        git_command(repo_root).args(["worktree", "list", "--porcelain"]),
        "Failed to list worktrees",
    )?;
    Ok(parse_worktree_list(&output))
}

/// Get the number of commits ahead and behind between two branches
/// Returns (ahead, behind) where ahead is commits in branch not in base,
/// and behind is commits in base not in branch
pub fn get_ahead_behind(worktree_path: &Path, branch: &str, base_branch: &str) -> (u32, u32) {
    let output = git_command(worktree_path)
        .args(["rev-list", "--left-right", "--count"])
        .arg(format!("{}...{}", base_branch, branch))
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = stdout.trim().split('\t').collect();
            if parts.len() == 2 {
                let behind = parts[0].parse().unwrap_or(0);
                let ahead = parts[1].parse().unwrap_or(0);
                (ahead, behind)
            } else {
                (0, 0)
            }
        }
        _ => (0, 0),
    }
}

/// Get the total lines added and removed between two branches
/// Returns (added, removed)
pub fn get_diff_stats(worktree_path: &Path, branch: &str, base_branch: &str) -> (u32, u32) {
    let output = git_command(worktree_path)
        .args(["diff", "--numstat"])
        .arg(format!("{}...{}", base_branch, branch))
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut added = 0u32;
            let mut removed = 0u32;
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    // Binary files show "-" instead of numbers
                    added += parts[0].parse::<u32>().unwrap_or(0);
                    removed += parts[1].parse::<u32>().unwrap_or(0);
                }
            }
            (added, removed)
        }
        _ => (0, 0),
    }
}

/// Check if a worktree has no uncommitted changes
#[must_use]
pub fn is_worktree_clean(path: &Path) -> bool {
    let output = git_command(path).args(["status", "--porcelain"]).output();

    match output {
        Ok(out) if out.status.success() => {
            out.stdout.is_empty() // Empty output means clean
        }
        _ => false, // If we can't check, assume not clean (safe default)
    }
}

/// Stats about uncommitted changes in a worktree
#[derive(Debug, Default, Clone, Copy)]
pub struct UncommittedStats {
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
}

impl UncommittedStats {
    pub fn is_clean(&self) -> bool {
        self.staged == 0 && self.modified == 0 && self.untracked == 0
    }

    pub fn total(&self) -> u32 {
        self.staged + self.modified + self.untracked
    }
}

/// Get stats about uncommitted changes in a worktree
pub fn get_uncommitted_stats(path: &Path) -> UncommittedStats {
    let output = git_command(path).args(["status", "--porcelain"]).output();

    let mut stats = UncommittedStats::default();

    let Ok(out) = output else {
        return stats;
    };
    if !out.status.success() {
        return stats;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if line.len() < 2 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');

        // Staged changes (index has changes)
        if index_status != ' ' && index_status != '?' {
            stats.staged += 1;
        }
        // Modified in worktree but not staged
        else if worktree_status != ' ' && worktree_status != '?' {
            stats.modified += 1;
        }
        // Untracked files
        else if index_status == '?' {
            stats.untracked += 1;
        }
    }

    stats
}

/// Get list of uncommitted file names in a worktree
pub fn get_uncommitted_files(path: &Path) -> Vec<String> {
    let output = git_command(path).args(["status", "--porcelain"]).output();

    let mut files = Vec::new();

    let Ok(out) = output else {
        return files;
    };
    if !out.status.success() {
        return files;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        // Porcelain format: XY filename
        // where X is index status, Y is worktree status
        if line.len() < 3 {
            continue;
        }
        // Skip the status chars and space
        let filename = &line[3..];
        // Handle renamed files (old -> new format)
        let filename = filename.split(" -> ").last().unwrap_or(filename);
        files.push(filename.to_string());
    }

    files
}

/// Check if a branch has been merged into the main branch
/// Uses merge-base to check if branch is an ancestor of main
#[must_use]
pub fn is_branch_merged(repo_root: &Path, branch: &str, main_branch: &str) -> bool {
    let output = git_command(repo_root)
        .args(["merge-base", "--is-ancestor", branch, main_branch])
        .output();

    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Delete a branch
pub fn delete_branch(repo_root: &Path, branch: &str) -> Result<(), String> {
    run_git(
        git_command(repo_root).args(["branch", "-d", branch]),
        "Failed to delete branch",
    )?;
    Ok(())
}

/// Delete a branch, printing git output
pub fn delete_branch_verbose(repo_root: &Path, branch: &str) -> Result<(), String> {
    run_git_verbose(
        git_command(repo_root).args(["branch", "-d", branch]),
        "Failed to delete branch",
    )?;
    Ok(())
}

/// Rebase the current branch onto another branch (quiet mode)
pub fn rebase_onto(worktree_path: &Path, base_branch: &str) -> Result<(), String> {
    run_git(
        git_command(worktree_path).args(["rebase", base_branch]),
        "Failed to rebase",
    )?;
    Ok(())
}

/// Rebase the current branch onto another branch with interactive output
pub fn rebase_onto_interactive(worktree_path: &Path, base_branch: &str) -> Result<(), String> {
    run_git_interactive(
        git_command(worktree_path).args(["rebase", base_branch]),
        "Failed to rebase",
    )
}

/// Merge a branch into the current branch (quiet mode)
pub fn merge_branch(worktree_path: &Path, branch: &str) -> Result<(), String> {
    run_git(
        git_command(worktree_path).args(["merge", branch]),
        "Failed to merge",
    )?;
    Ok(())
}

/// Merge a branch into the current branch with interactive output
pub fn merge_branch_interactive(worktree_path: &Path, branch: &str) -> Result<(), String> {
    run_git_interactive(
        git_command(worktree_path).args(["merge", branch]),
        "Failed to merge",
    )
}

/// Result of attempting to clean up a branch after worktree removal
pub struct BranchCleanupResult {
    pub was_merged: bool,
}

/// Clean up a branch by deleting it if it was merged into main.
pub fn cleanup_branch(repo_root: &Path, branch: &str, main_branch: &str) -> BranchCleanupResult {
    cleanup_branch_internal(repo_root, branch, main_branch, false)
}

/// Clean up a branch by deleting it if it was merged into main, with verbose output.
pub fn cleanup_branch_verbose(
    repo_root: &Path,
    branch: &str,
    main_branch: &str,
) -> BranchCleanupResult {
    cleanup_branch_internal(repo_root, branch, main_branch, true)
}

fn cleanup_branch_internal(
    repo_root: &Path,
    branch: &str,
    main_branch: &str,
    verbose: bool,
) -> BranchCleanupResult {
    let was_merged = is_branch_merged(repo_root, branch, main_branch);
    if was_merged {
        let delete_fn = if verbose {
            delete_branch_verbose
        } else {
            delete_branch
        };
        // Ignore deletion result - git outputs any errors
        let _ = delete_fn(repo_root, branch);
    }
    BranchCleanupResult { was_merged }
}

/// Get the main worktree path from a silo worktree by reading its .git file
pub fn get_main_worktree_from_silo(silo_path: &Path) -> Option<PathBuf> {
    let git_file = silo_path.join(".git");
    let content = std::fs::read_to_string(&git_file).ok()?;

    // .git file contains: gitdir: /path/to/.git/worktrees/branch-name
    let gitdir = content.strip_prefix("gitdir: ")?.trim();
    let gitdir_path = PathBuf::from(gitdir);

    // Go up from .git/worktrees/branch-name to .git, then get parent
    // Structure: /main/repo/.git/worktrees/branch -> /main/repo/.git -> /main/repo
    let git_dir = gitdir_path.parent()?.parent()?; // .git/worktrees -> .git
    let main_worktree = git_dir.parent()?; // .git -> repo root

    Some(main_worktree.to_path_buf())
}

fn parse_worktree_list(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut has_head = false;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            // Save previous worktree if exists
            if let Some(path) = current_path.take()
                && has_head
            {
                worktrees.push(Worktree {
                    path,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(PathBuf::from(path));
            has_head = false;
            current_branch = None;
        } else if line.starts_with("HEAD ") {
            has_head = true;
        } else if let Some(branch) = line.strip_prefix("branch ") {
            // Branch is in format refs/heads/branch-name
            let branch_name = branch.strip_prefix("refs/heads/").unwrap_or(branch);
            current_branch = Some(branch_name.to_string());
        }
    }

    // Don't forget the last worktree
    if let Some(path) = current_path
        && has_head
    {
        worktrees.push(Worktree {
            path,
            branch: current_branch,
        });
    }

    worktrees
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_repo_name_from_https_url() {
        assert_eq!(
            extract_repo_name_from_url("https://github.com/user/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn test_extract_repo_name_from_https_url_no_git_suffix() {
        assert_eq!(
            extract_repo_name_from_url("https://github.com/user/repo"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn test_extract_repo_name_from_ssh_url() {
        assert_eq!(
            extract_repo_name_from_url("git@github.com:user/repo.git"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn test_extract_repo_name_from_ssh_url_no_git_suffix() {
        assert_eq!(
            extract_repo_name_from_url("git@github.com:user/repo"),
            Some("repo".to_string())
        );
    }

    #[test]
    fn test_parse_worktree_list_empty() {
        let output = "";
        let worktrees = parse_worktree_list(output);
        assert!(worktrees.is_empty());
    }

    #[test]
    fn test_parse_worktree_list_single() {
        let output = "worktree /path/to/repo\nHEAD abc123\nbranch refs/heads/main\n";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/repo"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_worktree_list_multiple() {
        let output = "worktree /path/to/repo\nHEAD abc123\nbranch refs/heads/main\n\nworktree /path/to/silo\nHEAD def456\nbranch refs/heads/feature\n";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/repo"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(worktrees[1].path, PathBuf::from("/path/to/silo"));
        assert_eq!(worktrees[1].branch, Some("feature".to_string()));
    }

    #[test]
    fn test_parse_worktree_list_detached_head() {
        let output = "worktree /path/to/repo\nHEAD abc123\ndetached\n";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch, None);
    }
}
