//! The `prune` command: remove silos with no uncommitted changes.

use crate::git;
use crate::prompt;
use crate::removal;
use crate::silo;

pub fn run(all: bool, dry_run: bool, force: bool, quiet: bool) -> Result<(), String> {
    let repo_root = git::try_get_repo_root();

    let prunable = if all {
        silo::collect_prunable_all()?
    } else if let Some(ref root) = repo_root {
        silo::collect_prunable_repo(root)?
    } else {
        return Err(
            "Not in a git repository. Use --all to prune silos for all repositories.".to_string(),
        );
    };

    if prunable.is_empty() {
        if !quiet {
            println!("No clean silos to prune.");
        }
        return Ok(());
    }

    // Convert to RemovableSilo, partitioning into removable and blocked
    let (removable, blocked): (Vec<_>, Vec<_>) = if force {
        // With force, all silos are removable
        let removable: Vec<_> = prunable
            .into_iter()
            .map(removal::RemovableSilo::from_silo_unchecked)
            .collect();
        (removable, Vec::new())
    } else {
        // Without force, validate each silo
        let mut removable = Vec::new();
        let mut blocked = Vec::new();
        for silo in prunable {
            match removal::RemovableSilo::try_from(silo) {
                Ok(r) => removable.push(r),
                Err(e) => blocked.push(e),
            }
        }
        (removable, blocked)
    };

    if dry_run {
        for error in &blocked {
            println!("Would skip: {} (blocked)", error.silo.name);
        }
        for r in &removable {
            r.print_dry_run();
        }
        println!("\n{} silo(s) would be pruned.", removable.len());
        if !blocked.is_empty() {
            println!("{} silo(s) skipped due to blockers.", blocked.len());
        }
        return Ok(());
    }

    // Report skipped silos
    if !blocked.is_empty() && !quiet {
        eprintln!("Skipping {} silo(s) with blockers:", blocked.len());
        for error in &blocked {
            eprintln!("  {}", error.silo.name);
        }
    }

    if removable.is_empty() {
        if !quiet {
            println!("No silos to prune (after excluding blocked silos).");
        }
        return Ok(());
    }

    // Batch confirmation
    if !force {
        println!("Will prune {} silo(s):", removable.len());
        for r in &removable {
            println!("  {}", r.name());
        }
        if !prompt::confirm("Continue?") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Execute removals
    for r in removable {
        let name = r.name().to_string();
        if force {
            r.remove_force(quiet)?;
        } else {
            r.remove(quiet)?;
        }
        if !quiet {
            println!("Pruned: {}", name);
        }
    }

    Ok(())
}
