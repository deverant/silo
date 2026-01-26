//! Integration tests for silo CLI commands.
//!
//! These tests create temporary git repositories and silo directories
//! to test the CLI end-to-end.

mod common;

use common::TestEnv;
use std::fs;
use std::process::Command;

// =============================================================================
// NEW COMMAND TESTS
// =============================================================================

#[test]
fn test_new_creates_silo() {
    let env = TestEnv::new();

    let output = env.run_silo(&["new", "test-branch"]);

    TestEnv::assert_success(&output);
    env.assert_silo_exists("test-branch");
}

#[test]
fn test_new_fails_for_existing_silo() {
    let env = TestEnv::new();

    env.create_silo("duplicate");
    let output = env.run_silo(&["new", "duplicate"]);

    TestEnv::assert_failure(&output);
}

// =============================================================================
// LIST COMMAND TESTS
// =============================================================================

#[test]
fn test_list_shows_silos() {
    let env = TestEnv::new();
    env.create_silos(&["branch1", "branch2"]);

    let output = env.run_silo(&["list"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("branch1"), "Output should contain branch1");
    assert!(stdout.contains("branch2"), "Output should contain branch2");
}

#[test]
fn test_list_empty_repo() {
    let env = TestEnv::new();

    let output = env.run_silo(&["list"]);

    TestEnv::assert_success(&output);
}

#[test]
fn test_list_all_across_repos() {
    let env = TestEnv::new();
    env.create_silo("test-silo");

    let output = env.run_silo(&["list", "--all"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("test-silo"));
}

// =============================================================================
// RM COMMAND TESTS
// =============================================================================

#[test]
fn test_rm_removes_silo() {
    let env = TestEnv::new();
    env.create_silo("to-remove");
    env.assert_silo_exists("to-remove");

    let output = env.run_silo(&["rm", "to-remove", "--force"]);

    TestEnv::assert_success(&output);
    env.assert_silo_not_exists("to-remove");
}

#[test]
fn test_rm_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["rm", "nonexistent", "--force"]);

    TestEnv::assert_failure(&output);
}

#[test]
fn test_rm_with_uncommitted_changes_requires_force() {
    let env = TestEnv::new();
    env.create_silo("dirty-silo");
    env.create_uncommitted_file("dirty-silo", "dirty.txt", "uncommitted");

    // Should fail without --force
    let output = env.run_silo(&["rm", "dirty-silo"]);
    TestEnv::assert_failure(&output);
    env.assert_silo_exists("dirty-silo");

    // Should succeed with --force
    let output = env.run_silo(&["rm", "dirty-silo", "--force"]);
    TestEnv::assert_success(&output);
    env.assert_silo_not_exists("dirty-silo");
}

#[test]
fn test_rm_dry_run_does_not_remove() {
    let env = TestEnv::new();
    env.create_silo("keep-me");
    env.assert_silo_exists("keep-me");

    let output = env.run_silo(&["rm", "keep-me", "--dry-run", "--force"]);

    TestEnv::assert_success(&output);
    env.assert_silo_exists("keep-me"); // Should still exist
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("Would remove"));
}

// =============================================================================
// CD COMMAND TESTS
// =============================================================================

#[test]
fn test_cd_outputs_directive() {
    let env = TestEnv::new();
    env.create_silo("cd-test");

    let directive_file = env.silo_dir.path().join("directive");
    let output = env.run_silo_with_env(
        &["cd", "cd-test"],
        &[("SILO_DIRECTIVE_FILE", directive_file.to_str().unwrap())],
    );

    TestEnv::assert_success(&output);
    let directive = fs::read_to_string(&directive_file).expect("Should have written directive");
    assert!(directive.contains("cd="));
}

#[test]
fn test_cd_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["cd", "nonexistent"]);

    TestEnv::assert_failure(&output);
}

#[test]
fn test_cd_no_args_returns_main_worktree() {
    let env = TestEnv::new();
    env.create_silo("feature-branch");

    let directive_file = env.silo_dir.path().join("directive");
    let output = env.run_silo_with_env(
        &["cd"],
        &[("SILO_DIRECTIVE_FILE", directive_file.to_str().unwrap())],
    );

    TestEnv::assert_success(&output);
    let directive = fs::read_to_string(&directive_file).expect("Should have written directive");
    assert!(directive.contains("cd="));

    let repo_path = env.repo_dir.path().to_string_lossy();
    assert!(directive.contains(repo_path.as_ref()));
}

#[test]
fn test_cd_no_args_not_in_repo_fails() {
    let env = TestEnv::new();

    let output = Command::new(TestEnv::silo_bin())
        .args(["cd"])
        .current_dir(&env.silo_dir) // silo_dir is not a git repo
        .output()
        .expect("Failed to run silo cd");

    TestEnv::assert_failure(&output);
    let stderr = TestEnv::stderr(&output);
    assert!(stderr.contains("Not in a git repository"));
}

// =============================================================================
// EXEC COMMAND TESTS
// =============================================================================

#[test]
fn test_exec_runs_command_in_silo() {
    let env = TestEnv::new();
    env.create_silo("exec-test");

    let output = env.run_silo(&["exec", "exec-test", "pwd"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("exec-test"));
}

#[test]
fn test_exec_with_args() {
    let env = TestEnv::new();
    env.create_silo("exec-args");

    let output = env.run_silo(&["exec", "exec-args", "echo", "hello", "world"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("hello world"));
}

#[test]
fn test_exec_nonexistent_silo_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["exec", "nonexistent", "pwd"]);

    TestEnv::assert_failure(&output);
}

#[test]
fn test_exec_failing_command_returns_error() {
    let env = TestEnv::new();
    env.create_silo("exec-fail");

    let output = env.run_silo(&["exec", "exec-fail", "false"]);

    TestEnv::assert_failure(&output);
}

// =============================================================================
// PRUNE COMMAND TESTS
// =============================================================================

#[test]
fn test_prune_removes_clean_silos() {
    let env = TestEnv::new();
    env.create_silos(&["clean1", "clean2"]);

    let output = env.run_silo(&["prune", "--force"]);

    TestEnv::assert_success(&output);
    env.assert_silo_not_exists("clean1");
    env.assert_silo_not_exists("clean2");
}

#[test]
fn test_prune_preserves_dirty_silos() {
    let env = TestEnv::new();
    env.create_silos(&["clean", "dirty"]);
    env.create_uncommitted_file("dirty", "file.txt", "content");

    let output = env.run_silo(&["prune", "--force"]);

    TestEnv::assert_success(&output);
    env.assert_silo_not_exists("clean");
    env.assert_silo_exists("dirty");
}

#[test]
fn test_prune_with_force_removes_silos_with_commits() {
    let env = TestEnv::new();
    env.create_silos(&["clean", "has-commits"]);
    env.create_commit_in_silo("has-commits", "new.txt", "content", "Add new file");

    // With --force, even silos with unmerged commits are removed
    let output = env.run_silo(&["prune", "--force"]);

    TestEnv::assert_success(&output);
    env.assert_silo_not_exists("clean");
    env.assert_silo_not_exists("has-commits");
}

#[test]
fn test_prune_skips_silos_with_unmerged_commits() {
    let env = TestEnv::new();
    env.create_silos(&["clean", "has-commits"]);
    env.create_commit_in_silo("has-commits", "new.txt", "content", "Add new file");

    // Without --force, silos with unmerged commits should be blocked
    // Since we're non-interactive, it will abort asking for confirmation
    let output = env.run_silo(&["prune"]);

    TestEnv::assert_success(&output);
    // "has-commits" should be skipped and reported as blocked
    let stderr = TestEnv::stderr(&output);
    assert!(
        stderr.contains("Skipping") || stderr.contains("blocked"),
        "Should report skipped silos"
    );
    // Both should still exist since confirmation was denied
    env.assert_silo_exists("clean");
    env.assert_silo_exists("has-commits");
}

#[test]
fn test_prune_dry_run() {
    let env = TestEnv::new();
    env.create_silo("clean");

    let output = env.run_silo(&["prune", "--dry-run"]);

    TestEnv::assert_success(&output);
    env.assert_silo_exists("clean"); // Should still exist
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("Would remove"));
}

#[test]
fn test_prune_empty_repo() {
    let env = TestEnv::new();

    let output = env.run_silo(&["prune", "--force"]);

    TestEnv::assert_success(&output);
}

#[test]
fn test_prune_all() {
    let env = TestEnv::new();
    env.create_silo("clean");

    let output = env.run_silo(&["prune", "--all", "--force"]);

    TestEnv::assert_success(&output);
    env.assert_silo_not_exists("clean");
}

// =============================================================================
// REBASE COMMAND TESTS
// =============================================================================

#[test]
fn test_rebase_success() {
    let env = TestEnv::new();
    env.create_silo("feature");

    // Create a commit in the silo
    env.create_commit_in_silo("feature", "feature.txt", "feature content", "Add feature");

    // Create a commit in main that doesn't conflict
    env.create_commit("main.txt", "main content", "Add to main");

    let output = env.run_silo(&["rebase", "feature"]);

    TestEnv::assert_success(&output);
}

#[test]
fn test_rebase_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["rebase", "nonexistent"]);

    TestEnv::assert_failure(&output);
}

#[test]
fn test_rebase_dry_run() {
    let env = TestEnv::new();
    env.create_silo("feature");
    env.create_commit_in_silo("feature", "feature.txt", "content", "Feature");
    env.create_commit("main.txt", "content", "Main");

    let output = env.run_silo(&["rebase", "feature", "--dry-run"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("Would rebase"));
}

// =============================================================================
// MERGE COMMAND TESTS
// =============================================================================

#[test]
fn test_merge_success() {
    let env = TestEnv::new();
    env.create_silo("feature");

    // Create a commit in the silo
    env.create_commit_in_silo("feature", "feature.txt", "feature content", "Add feature");

    let output = env.run_silo(&["merge", "feature"]);

    TestEnv::assert_success(&output);

    // Verify the file is now in the main repo
    let feature_file = env.repo_dir.path().join("feature.txt");
    assert!(feature_file.exists(), "Merged file should exist in main");
}

#[test]
fn test_merge_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["merge", "nonexistent"]);

    TestEnv::assert_failure(&output);
}

#[test]
fn test_merge_dry_run() {
    let env = TestEnv::new();
    env.create_silo("feature");
    env.create_commit_in_silo("feature", "feature.txt", "content", "Feature");

    let output = env.run_silo(&["merge", "feature", "--dry-run"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("Would merge"));

    // File should not exist (dry run)
    let feature_file = env.repo_dir.path().join("feature.txt");
    assert!(!feature_file.exists());
}

#[test]
fn test_merge_from_silo_fails() {
    let env = TestEnv::new();
    env.create_silo("feature");

    // Try to run merge from inside the silo (should fail - must run from main)
    let output = env.run_silo_in("feature", &["merge", "feature"]);

    TestEnv::assert_failure(&output);
}

// =============================================================================
// RESET COMMAND TESTS
// =============================================================================

#[test]
fn test_reset_success() {
    let env = TestEnv::new();
    env.create_silo("feature");

    // Create a commit in the silo
    env.create_commit_in_silo("feature", "feature.txt", "feature content", "Add feature");

    // Create a commit in main
    env.create_commit("main.txt", "main content", "Update main");

    // Reset the silo to main's HEAD
    let output = env.run_silo(&["reset", "feature", "--force"]);

    TestEnv::assert_success(&output);

    // The silo should now have main.txt (from main's current commit)
    let main_file = env.silo_path("feature").join("main.txt");
    assert!(main_file.exists(), "Silo should have main.txt after reset");

    // The feature.txt should be gone (it was only in the silo's commits)
    let feature_file = env.silo_path("feature").join("feature.txt");
    assert!(
        !feature_file.exists(),
        "feature.txt should be gone after reset"
    );
}

#[test]
fn test_reset_nonexistent_fails() {
    let env = TestEnv::new();

    let output = env.run_silo(&["reset", "nonexistent", "--force"]);

    TestEnv::assert_failure(&output);
}

#[test]
fn test_reset_dry_run() {
    let env = TestEnv::new();
    env.create_silo("feature");
    env.create_commit_in_silo("feature", "feature.txt", "content", "Feature");

    let output = env.run_silo(&["reset", "feature", "--dry-run", "--force"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(stdout.contains("Would reset"));

    // File should still exist (dry run)
    let feature_file = env.silo_path("feature").join("feature.txt");
    assert!(
        feature_file.exists(),
        "File should still exist after dry run"
    );
}

#[test]
fn test_reset_with_uncommitted_changes_requires_confirmation() {
    let env = TestEnv::new();
    env.create_silo("dirty-silo");
    env.create_uncommitted_file("dirty-silo", "dirty.txt", "uncommitted");

    // Without --force, should ask for confirmation (fails in non-interactive)
    let output = env.run_silo(&["reset", "dirty-silo"]);
    // In non-interactive mode, confirmation defaults to no
    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(
        stdout.contains("Aborted"),
        "Should abort without confirmation"
    );

    // File should still exist
    let dirty_file = env.silo_path("dirty-silo").join("dirty.txt");
    assert!(dirty_file.exists(), "dirty.txt should still exist");
}

#[test]
fn test_reset_with_force_discards_uncommitted_changes() {
    let env = TestEnv::new();
    env.create_silo("dirty-silo");
    env.create_uncommitted_file("dirty-silo", "dirty.txt", "uncommitted");

    // With --force, should reset without asking
    let output = env.run_silo(&["reset", "dirty-silo", "--force"]);

    TestEnv::assert_success(&output);

    // dirty.txt should be gone
    let dirty_file = env.silo_path("dirty-silo").join("dirty.txt");
    assert!(
        !dirty_file.exists(),
        "dirty.txt should be removed after reset"
    );
}

#[test]
fn test_reset_with_unmerged_commits_requires_confirmation() {
    let env = TestEnv::new();
    env.create_silo("has-commits");
    env.create_commit_in_silo("has-commits", "new.txt", "content", "Add new file");

    // Without --force, should ask for confirmation (fails in non-interactive)
    let output = env.run_silo(&["reset", "has-commits"]);
    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    assert!(
        stdout.contains("Aborted"),
        "Should abort without confirmation"
    );

    // File should still exist
    let new_file = env.silo_path("has-commits").join("new.txt");
    assert!(new_file.exists(), "new.txt should still exist");
}

#[test]
fn test_reset_with_force_discards_unmerged_commits() {
    let env = TestEnv::new();
    env.create_silo("has-commits");
    env.create_commit_in_silo("has-commits", "new.txt", "content", "Add new file");

    // With --force, should reset without asking
    let output = env.run_silo(&["reset", "has-commits", "--force"]);

    TestEnv::assert_success(&output);

    // new.txt should be gone
    let new_file = env.silo_path("has-commits").join("new.txt");
    assert!(!new_file.exists(), "new.txt should be removed after reset");
}

#[test]
fn test_reset_quiet_flag() {
    let env = TestEnv::new();
    env.create_silo("quiet-reset");
    env.create_commit("main.txt", "content", "Add main file");

    let output = env.run_silo(&["reset", "quiet-reset", "--force", "--quiet"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    // Output should be minimal
    assert!(
        stdout.trim().is_empty(),
        "Quiet reset should produce no stdout"
    );
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[test]
fn test_silo_name_with_slashes() {
    let env = TestEnv::new();

    let output = env.run_silo(&["new", "feature/sub-feature"]);

    TestEnv::assert_success(&output);
    env.assert_silo_exists("feature/sub-feature");
}

#[test]
fn test_quiet_flag_suppresses_output() {
    let env = TestEnv::new();

    let output = env.run_silo(&["new", "quiet-test", "--quiet"]);

    TestEnv::assert_success(&output);
    let stdout = TestEnv::stdout(&output);
    // Output should be minimal/empty
    assert!(stdout.trim().is_empty() || stdout.len() < 50);
}
