//! Integration tests for silo CLI commands.
//!
//! These tests create temporary git repositories and silo directories
//! to test the CLI end-to-end.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Test environment with temporary directories for repo and silo storage.
struct TestEnv {
    /// Temporary directory for silo worktree storage
    silo_dir: TempDir,
    /// Temporary directory for the git repository
    repo_dir: TempDir,
}

impl TestEnv {
    /// Create a new test environment with an initialized git repo.
    fn new() -> Self {
        let silo_dir = TempDir::new().expect("Failed to create silo temp dir");
        let repo_dir = TempDir::new().expect("Failed to create repo temp dir");

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_dir)
            .output()
            .expect("Failed to init git repo");

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_dir)
            .output()
            .expect("Failed to configure git email");

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_dir)
            .output()
            .expect("Failed to configure git user");

        // Create initial commit (required for worktrees)
        fs::write(repo_dir.path().join("README.md"), "# Test Repo\n")
            .expect("Failed to write README");

        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_dir)
            .output()
            .expect("Failed to git add");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_dir)
            .output()
            .expect("Failed to git commit");

        // Create local config pointing to our temp silo dir
        let config = format!("worktree_dir = \"{}\"", silo_dir.path().display());
        fs::write(repo_dir.path().join(".silo.toml"), config).expect("Failed to write config");

        Self { silo_dir, repo_dir }
    }

    /// Run silo command with given arguments.
    fn run_silo(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_silo"))
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .expect("Failed to run silo command")
    }

    /// Check if a silo directory exists in the storage.
    fn silo_exists(&self, name: &str) -> bool {
        // Silo directories are stored as {repo_name}-{hash}/{name}
        if let Ok(entries) = fs::read_dir(self.silo_dir.path()) {
            for entry in entries.flatten() {
                let silo_path = entry.path().join(name);
                if silo_path.exists() {
                    return true;
                }
            }
        }
        false
    }
}

#[test]
fn test_new_creates_silo() {
    let env = TestEnv::new();

    let output = env.run_silo(&["new", "test-branch"]);

    assert!(
        output.status.success(),
        "silo new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify silo was created
    assert!(
        env.silo_exists("test-branch"),
        "Silo directory was not created"
    );
}

#[test]
fn test_new_fails_for_existing_silo() {
    let env = TestEnv::new();

    // Create first silo
    let output1 = env.run_silo(&["new", "duplicate"]);
    assert!(output1.status.success());

    // Try to create duplicate
    let output2 = env.run_silo(&["new", "duplicate"]);
    assert!(
        !output2.status.success(),
        "silo new should fail for duplicate"
    );
}

#[test]
fn test_list_shows_silos() {
    let env = TestEnv::new();

    // Create some silos
    env.run_silo(&["new", "branch1"]);
    env.run_silo(&["new", "branch2"]);

    let output = env.run_silo(&["list"]);

    assert!(
        output.status.success(),
        "silo list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("branch1"), "Output should contain branch1");
    assert!(stdout.contains("branch2"), "Output should contain branch2");
}

#[test]
fn test_list_empty_repo() {
    let env = TestEnv::new();

    let output = env.run_silo(&["list"]);

    assert!(
        output.status.success(),
        "silo list should succeed even with no silos"
    );
}

#[test]
fn test_rm_removes_silo() {
    let env = TestEnv::new();

    // Create a silo
    env.run_silo(&["new", "to-remove"]);
    assert!(env.silo_exists("to-remove"), "Silo should exist after new");

    // Remove it
    let output = env.run_silo(&["rm", "to-remove", "--force"]);

    assert!(
        output.status.success(),
        "silo rm failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify removal
    assert!(
        !env.silo_exists("to-remove"),
        "Silo should not exist after rm"
    );
}

#[test]
fn test_rm_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["rm", "nonexistent", "--force"]);

    assert!(
        !output.status.success(),
        "silo rm should fail for nonexistent silo"
    );
}

#[test]
fn test_cd_outputs_directive() {
    let env = TestEnv::new();

    // Create a silo
    env.run_silo(&["new", "cd-test"]);

    // Create a temp file for directive output
    let directive_file = env.silo_dir.path().join("directive");

    let output = Command::new(env!("CARGO_BIN_EXE_silo"))
        .args(["cd", "cd-test"])
        .current_dir(&env.repo_dir)
        .env("SILO_DIRECTIVE_FILE", &directive_file)
        .output()
        .expect("Failed to run silo cd");

    assert!(
        output.status.success(),
        "silo cd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check directive file was written
    let directive =
        fs::read_to_string(&directive_file).expect("Should have written directive file");
    assert!(
        directive.contains("cd="),
        "Directive should contain cd command"
    );
}

#[test]
fn test_cd_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["cd", "nonexistent"]);

    assert!(
        !output.status.success(),
        "silo cd should fail for nonexistent silo"
    );
}

#[test]
fn test_cd_no_args_returns_main_worktree() {
    let env = TestEnv::new();

    // Create a silo and get its path
    env.run_silo(&["new", "feature-branch"]);

    // Create a temp file for directive output
    let directive_file = env.silo_dir.path().join("directive");

    // Run cd without arguments from the main repo
    let output = Command::new(env!("CARGO_BIN_EXE_silo"))
        .args(["cd"])
        .current_dir(&env.repo_dir)
        .env("SILO_DIRECTIVE_FILE", &directive_file)
        .output()
        .expect("Failed to run silo cd");

    assert!(
        output.status.success(),
        "silo cd (no args) failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check directive file was written with main worktree path
    let directive =
        fs::read_to_string(&directive_file).expect("Should have written directive file");
    assert!(
        directive.contains("cd="),
        "Directive should contain cd command"
    );

    // The path in the directive should be the main repo path
    let repo_path = env.repo_dir.path().to_string_lossy();
    assert!(
        directive.contains(repo_path.as_ref()),
        "Directive should contain main worktree path: {}",
        directive
    );
}

#[test]
fn test_cd_no_args_not_in_repo_fails() {
    let env = TestEnv::new();

    // Run cd without arguments from a non-git directory
    let output = Command::new(env!("CARGO_BIN_EXE_silo"))
        .args(["cd"])
        .current_dir(&env.silo_dir) // silo_dir is not a git repo
        .output()
        .expect("Failed to run silo cd");

    assert!(
        !output.status.success(),
        "silo cd (no args) should fail outside a git repo"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Not in a git repository"),
        "Error should mention not being in a git repository: {}",
        stderr
    );
}

#[test]
fn test_list_all_across_repos() {
    let env = TestEnv::new();

    // Create a silo
    env.run_silo(&["new", "test-silo"]);

    let output = env.run_silo(&["list", "--all"]);

    assert!(
        output.status.success(),
        "silo list --all failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test-silo"),
        "Output should contain test-silo"
    );
}
