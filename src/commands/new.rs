//! The `new` command: create a new silo with a new branch.

use crate::git;
use crate::shell;
use crate::silo;

use super::run_command_in_dir;

pub fn run(branch: String, command: &[String], dry_run: bool, quiet: bool) -> Result<(), String> {
    let repo_info = git::get_repo_info()?;
    let repo_root = &repo_info.main_worktree;
    let silo_path = silo::get_silo_path(&repo_info.name, repo_root, &branch)?;

    if dry_run {
        println!("Would create silo at: {}", silo_path.display());
        println!("Would create branch: {}", branch);
        if !command.is_empty() {
            println!("Would execute: {}", command.join(" "));
        }
        return Ok(());
    }

    // Create parent directories if needed
    if let Some(parent) = silo_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create silo directory: {}", e))?;
    }

    if !quiet {
        println!("Creating branch '{}'...", branch);
        git::create_worktree_verbose(&silo_path, &branch, repo_root)?;
        println!("Created silo: {}", silo_path.display());
    } else {
        git::create_worktree(&silo_path, &branch, repo_root)?;
    }

    // Track this silo as the last used
    shell::write_directive("last", &branch);

    // Execute command in the new silo if provided
    if !command.is_empty() {
        run_command_in_dir(command, &silo_path)?;
        if !quiet {
            eprintln!("[silo: {}]", branch);
        }
    }

    Ok(())
}
