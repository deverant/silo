//! Test utilities for silo integration tests.
//!
//! This module provides the `TestEnv` struct with helper methods for setting up
//! test environments, running silo commands, and asserting silo state.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Test environment with temporary directories for repo and silo storage.
pub struct TestEnv {
    /// Temporary directory for silo worktree storage
    pub silo_dir: TempDir,
    /// Temporary directory for the git repository
    pub repo_dir: TempDir,
}

impl TestEnv {
    /// Create a new test environment with an initialized git repo.
    pub fn new() -> Self {
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

        let env = Self { silo_dir, repo_dir };

        // Create local config pointing to our temp silo dir
        let config = format!("worktree_dir = \"{}\"", env.silo_dir.path().display());
        fs::write(env.repo_dir.path().join(".silo.toml"), config).expect("Failed to write config");

        env
    }

    /// Get the path to the silo binary.
    pub fn silo_bin() -> String {
        env!("CARGO_BIN_EXE_silo").to_string()
    }

    /// Run silo command with given arguments.
    pub fn run_silo(&self, args: &[&str]) -> Output {
        Command::new(Self::silo_bin())
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .expect("Failed to run silo command")
    }

    /// Run silo command with given arguments from a specific silo directory.
    pub fn run_silo_in(&self, silo_name: &str, args: &[&str]) -> Output {
        let silo_path = self.silo_path(silo_name);
        Command::new(Self::silo_bin())
            .args(args)
            .current_dir(&silo_path)
            .output()
            .expect("Failed to run silo command")
    }

    /// Run silo command with given arguments and environment variables.
    pub fn run_silo_with_env(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(Self::silo_bin());
        cmd.args(args).current_dir(&self.repo_dir);
        for (key, value) in envs {
            cmd.env(key, value);
        }
        cmd.output().expect("Failed to run silo command")
    }

    /// Run a git command in the main repo.
    pub fn git(&self, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(&self.repo_dir)
            .output()
            .expect("Failed to run git command")
    }

    /// Run a git command in a silo.
    pub fn git_in_silo(&self, silo_name: &str, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(self.silo_path(silo_name))
            .output()
            .expect("Failed to run git command")
    }

    /// Create a silo with the given name.
    pub fn create_silo(&self, name: &str) {
        let output = self.run_silo(&["new", name]);
        assert!(
            output.status.success(),
            "Failed to create silo {}: {}",
            name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Create multiple silos.
    pub fn create_silos(&self, names: &[&str]) {
        for name in names {
            self.create_silo(name);
        }
    }

    /// Get the full path to a silo directory.
    pub fn silo_path(&self, name: &str) -> PathBuf {
        // Silo directories are stored as {repo_name}-{hash}/{name}
        if let Ok(entries) = fs::read_dir(self.silo_dir.path()) {
            for entry in entries.flatten() {
                let silo_path = entry.path().join(name);
                if silo_path.exists() {
                    return silo_path;
                }
            }
        }
        // Return the expected path even if it doesn't exist
        self.silo_dir.path().join(name)
    }

    /// Check if a silo directory exists in the storage.
    pub fn silo_exists(&self, name: &str) -> bool {
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

    /// Assert that a silo exists.
    pub fn assert_silo_exists(&self, name: &str) {
        assert!(
            self.silo_exists(name),
            "Silo '{}' should exist but doesn't",
            name
        );
    }

    /// Assert that a silo does not exist.
    pub fn assert_silo_not_exists(&self, name: &str) {
        assert!(
            !self.silo_exists(name),
            "Silo '{}' should not exist but does",
            name
        );
    }

    /// Create a commit in the main repo with a file change.
    pub fn create_commit(&self, file: &str, content: &str, message: &str) {
        fs::write(self.repo_dir.path().join(file), content).expect("Failed to write file");
        self.git(&["add", file]);
        self.git(&["commit", "-m", message]);
    }

    /// Create a commit in a silo with a file change.
    pub fn create_commit_in_silo(&self, silo_name: &str, file: &str, content: &str, message: &str) {
        let silo_path = self.silo_path(silo_name);
        fs::write(silo_path.join(file), content).expect("Failed to write file");
        self.git_in_silo(silo_name, &["add", file]);
        self.git_in_silo(silo_name, &["commit", "-m", message]);
    }

    /// Create an uncommitted file in a silo.
    pub fn create_uncommitted_file(&self, silo_name: &str, file: &str, content: &str) {
        let silo_path = self.silo_path(silo_name);
        fs::write(silo_path.join(file), content).expect("Failed to write file");
    }

    /// Check if the output indicates success.
    pub fn assert_success(output: &Output) {
        assert!(
            output.status.success(),
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Check if the output indicates failure.
    pub fn assert_failure(output: &Output) {
        assert!(
            !output.status.success(),
            "Command should have failed but succeeded"
        );
    }

    /// Get stdout as a string.
    pub fn stdout(output: &Output) -> String {
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Get stderr as a string.
    pub fn stderr(output: &Output) -> String {
        String::from_utf8_lossy(&output.stderr).to_string()
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    /// Create an orphaned silo by creating a fake silo that points to a non-existent main worktree.
    /// Returns the path to the orphaned silo directory.
    pub fn create_orphaned_silo(&self, name: &str) -> PathBuf {
        // First create a real silo so we have the directory structure
        self.create_silo(name);
        let silo_path = self.silo_path(name);

        // Modify the .git file to point to a non-existent main worktree
        let git_file = silo_path.join(".git");
        let fake_main = PathBuf::from("/tmp/nonexistent-repo-12345/.git/worktrees").join(name);
        let content = format!("gitdir: {}", fake_main.display());
        fs::write(&git_file, content).expect("Failed to write .git file");

        // Now we need to unregister this worktree from the main repo since git still tracks it
        // Just remove it from git's perspective by removing the worktree reference in .git/worktrees
        let worktree_ref = self.repo_dir.path().join(".git/worktrees").join(name);
        if worktree_ref.exists() {
            let _ = fs::remove_dir_all(&worktree_ref);
        }

        silo_path
    }

    /// Create an empty repo directory in the silo storage.
    /// Returns the path to the empty directory.
    pub fn create_empty_repo_dir(&self, name: &str) -> PathBuf {
        let empty_dir = self.silo_dir.path().join(name);
        fs::create_dir_all(&empty_dir).expect("Failed to create empty repo dir");
        empty_dir
    }
}
