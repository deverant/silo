//! The `prune` command: remove silos with no uncommitted changes.

use crate::git;
use crate::names;
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

    // Generate display names - use repo prefix when pruning across all repos
    let all_silos: Vec<_> = removable
        .iter()
        .map(|r| r.silo().clone())
        .chain(blocked.iter().map(|e| e.silo.clone()))
        .collect();
    let display_names = names::generate_display_names(&all_silos, all);
    let display_name_map: std::collections::HashMap<_, _> = all_silos
        .iter()
        .zip(display_names.iter())
        .map(|(silo, name)| (silo.storage_path.clone(), name.clone()))
        .collect();

    // Helper to get display name for a silo
    let get_display_name = |silo: &silo::Silo| -> String {
        display_name_map
            .get(&silo.storage_path)
            .cloned()
            .unwrap_or_else(|| silo.name.clone())
    };

    if dry_run {
        for error in &blocked {
            println!("Would skip: {} (blocked)", get_display_name(&error.silo));
        }
        for r in &removable {
            let display_name = get_display_name(r.silo());
            println!("Would remove silo: {}", display_name);
            println!("  Path: {}", r.silo().storage_path.display());
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
            eprintln!("  {}", get_display_name(&error.silo));
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
            println!("  {}", get_display_name(r.silo()));
        }
        if !prompt::confirm("Continue?") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Execute removals
    for r in removable {
        let display_name = get_display_name(r.silo());
        r.remove(force, quiet)?;
        if !quiet {
            println!("Pruned: {}", display_name);
        }
    }

    Ok(())
}
