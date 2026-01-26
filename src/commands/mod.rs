//! CLI command implementations.
//!
//! Each subcommand is implemented in its own module for easier parallel development.

pub mod cd;
pub mod exec;
pub mod gc;
pub mod list;
pub mod merge;
pub mod new;
pub mod prune;
pub mod rebase;
pub mod reset;
pub mod rm;
pub mod sandbox;
pub mod shell;

use crate::process;
use crate::shell as shell_integration;

/// Resolve "-" to the last used silo from SILO_LAST environment variable.
/// Returns the name unchanged if it's not "-".
pub fn resolve_dash(name: &str) -> Result<String, String> {
    resolve_dash_with_last(name, std::env::var(shell_integration::LAST_ENV).ok())
}

/// Resolve "-" to the last used silo.
/// Returns the name unchanged if it's not "-".
fn resolve_dash_with_last(name: &str, last_silo: Option<String>) -> Result<String, String> {
    if name != "-" {
        return Ok(name.to_string());
    }

    last_silo.ok_or_else(|| "No previous silo. Use a silo name instead of '-'.".to_string())
}

/// Run a command in a specific directory, inheriting stdin/stdout/stderr.
/// Tracks the process while running so other commands can see it.
/// Exits the process if the command fails.
pub fn run_command_in_dir(command: &[String], dir: &std::path::Path) -> Result<(), String> {
    let (cmd, args) = command.split_first().ok_or("No command specified")?;

    let mut child = std::process::Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let pid = child.id();
    let command_str = command.join(" ");

    // Register the process for tracking
    if let Err(e) = process::register(dir, pid, &command_str) {
        eprintln!("Warning: Failed to register process: {}", e);
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for command: {}", e))?;

    // Unregister the process
    if let Err(e) = process::unregister(dir, pid) {
        eprintln!("Warning: Failed to unregister process: {}", e);
    }

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// Resolve a user-provided name to a silo.
pub fn resolve_silo(name: &str) -> Result<crate::silo::Silo, String> {
    let silos = crate::silo::collect_all_silos()?;

    if silos.is_empty() {
        return Err("No silos found.".to_string());
    }

    let current_repo = crate::git::try_get_repo_root();
    let result = crate::names::resolve_name(name, &silos, current_repo);

    match result {
        crate::names::ResolveResult::Found(silo) => Ok(silo.clone()),
        crate::names::ResolveResult::NotFound => Err(format!("Silo not found: {}", name)),
        crate::names::ResolveResult::Ambiguous(matches) => {
            // Generate display names with repo prefix for clarity
            let display_names = crate::names::generate_display_names(&silos, true);
            let ambiguous: Vec<String> = matches
                .iter()
                .filter_map(|m| {
                    silos
                        .iter()
                        .position(|s| s == *m)
                        .and_then(|idx| display_names.get(idx).cloned())
                })
                .collect();
            Err(format!(
                "Ambiguous silo name '{}'. Did you mean one of:\n  {}",
                name,
                ambiguous.join("\n  ")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_dash_returns_env_value() {
        assert_eq!(
            resolve_dash_with_last("-", Some("feature-branch".to_string())).unwrap(),
            "feature-branch"
        );
    }

    #[test]
    fn test_resolve_dash_passes_through_normal_names() {
        assert_eq!(
            resolve_dash_with_last("my-branch", None).unwrap(),
            "my-branch"
        );
        assert_eq!(
            resolve_dash_with_last("repo/branch", Some("ignored".to_string())).unwrap(),
            "repo/branch"
        );
    }

    #[test]
    fn test_resolve_dash_errors_when_unset() {
        let result = resolve_dash_with_last("-", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No previous silo"));
    }
}
