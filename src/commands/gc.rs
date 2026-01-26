//! The `gc` command: garbage collect orphaned silos and empty directories.

use crate::prompt;
use crate::silo;
use std::fs;

pub fn run(dry_run: bool, force: bool, quiet: bool) -> Result<(), String> {
    // Collect orphaned silos and empty directories
    let orphaned_silos = silo::collect_orphaned_silos()?;
    let empty_dirs = silo::collect_empty_repo_dirs()?;

    let total_orphaned = orphaned_silos.len();
    let total_empty = empty_dirs.len();

    if total_orphaned == 0 && total_empty == 0 {
        if !quiet {
            println!("No orphaned silos or empty directories to clean up.");
        }
        return Ok(());
    }

    // Report what we found
    if !quiet || dry_run {
        if total_orphaned > 0 {
            println!("Found {} orphaned silo(s):", total_orphaned);
            for orphan in &orphaned_silos {
                println!("  {}", orphan.storage_path.display());
                println!(
                    "    (main worktree missing: {})",
                    orphan.missing_main_worktree.display()
                );
            }
        }

        if total_empty > 0 {
            println!(
                "Found {} empty repo director{}:",
                total_empty,
                if total_empty == 1 { "y" } else { "ies" }
            );
            for dir in &empty_dirs {
                println!("  {}", dir.display());
            }
        }
    }

    if dry_run {
        println!(
            "\nWould remove {} orphaned silo(s) and {} empty director{}.",
            total_orphaned,
            total_empty,
            if total_empty == 1 { "y" } else { "ies" }
        );
        return Ok(());
    }

    // Confirm before proceeding
    if !force {
        let message = format!(
            "Remove {} orphaned silo(s) and {} empty director{}?",
            total_orphaned,
            total_empty,
            if total_empty == 1 { "y" } else { "ies" }
        );
        if !prompt::confirm(&message) {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Remove orphaned silos
    let mut removed_silos = 0;
    for orphan in orphaned_silos {
        if let Err(e) = fs::remove_dir_all(&orphan.storage_path) {
            eprintln!(
                "Warning: Failed to remove orphaned silo {}: {}",
                orphan.storage_path.display(),
                e
            );
        } else {
            removed_silos += 1;
            if !quiet {
                println!("Removed orphaned silo: {}", orphan.storage_path.display());
            }
        }
    }

    // Remove empty directories
    let mut removed_dirs = 0;
    for dir in empty_dirs {
        if let Err(e) = fs::remove_dir_all(&dir) {
            eprintln!(
                "Warning: Failed to remove empty directory {}: {}",
                dir.display(),
                e
            );
        } else {
            removed_dirs += 1;
            if !quiet {
                println!("Removed empty directory: {}", dir.display());
            }
        }
    }

    if !quiet {
        println!(
            "\nCleaned up {} orphaned silo(s) and {} empty director{}.",
            removed_silos,
            removed_dirs,
            if removed_dirs == 1 { "y" } else { "ies" }
        );
    }

    Ok(())
}
