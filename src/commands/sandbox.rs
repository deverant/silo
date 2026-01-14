//! Sandbox commands: run agents in Docker containers.

use crate::sandbox;
use crate::silo;

use super::resolve_silo;

/// Run Claude Code in a Docker sandbox.
pub fn claude(silo_name: Option<String>, dry_run: bool, args: &[String]) -> Result<(), String> {
    // Resolve workspace path
    let workspace = match silo_name {
        Some(name) => {
            let silo_info = resolve_silo(&name)?;
            silo_info.storage_path
        }
        None => {
            // Check if current directory is a silo
            let cwd = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;

            if !silo::is_silo_path(&cwd) {
                return Err("Not in a silo. Specify a silo name or navigate to one.".to_string());
            }
            cwd
        }
    };

    let config = sandbox::DockerSandboxConfig::claude(&workspace, args.to_vec());

    if dry_run {
        config.print();
    } else {
        config.run(&workspace)?;
    }

    Ok(())
}
