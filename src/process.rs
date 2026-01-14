//! Process tracking for silos.
//!
//! This module handles tracking of processes started in silos so that:
//! - The `list` command can show active process counts
//! - The `rm` command can warn before deleting silos with active processes

use std::fs;
use std::path::{Path, PathBuf};

/// Information about a tracked process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub command: String,
}

/// Get the tracking directory for a silo (parallel to worktree).
/// Maps: ~/.local/var/silo/repo-hash/branch -> ~/.local/var/silo/repo-hash/.tracking/branch
pub fn tracking_dir(silo_path: &Path) -> PathBuf {
    let parent = silo_path.parent().unwrap_or(silo_path);
    let branch = silo_path.file_name().unwrap_or_default();
    parent.join(".tracking").join(branch)
}

/// Get the pids directory inside the tracking directory.
pub fn pids_dir(silo_path: &Path) -> PathBuf {
    tracking_dir(silo_path).join("pids")
}

/// Register a process for tracking (creates PID file).
pub fn register(silo_path: &Path, pid: u32, command: &str) -> Result<(), String> {
    let dir = pids_dir(silo_path);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create tracking directory: {}", e))?;

    let pid_file = dir.join(pid.to_string());
    let content = format!("command={}\n", command);
    fs::write(&pid_file, content).map_err(|e| format!("Failed to write PID file: {}", e))?;

    Ok(())
}

/// Unregister a process (removes PID file).
pub fn unregister(silo_path: &Path, pid: u32) -> Result<(), String> {
    let pid_file = pids_dir(silo_path).join(pid.to_string());
    if pid_file.exists() {
        fs::remove_file(&pid_file).map_err(|e| format!("Failed to remove PID file: {}", e))?;
    }
    Ok(())
}

/// Get all active processes for a silo.
/// Reads PID files, checks if each process is still running, prunes dead entries.
pub fn list_active(silo_path: &Path) -> Vec<ProcessInfo> {
    let dir = pids_dir(silo_path);
    if !dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut active = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let Ok(pid) = filename.parse::<u32>() else {
            continue;
        };

        if is_running(pid) {
            let command = read_command(&path).unwrap_or_default();
            active.push(ProcessInfo { pid, command });
        } else {
            // Prune stale PID file
            let _ = fs::remove_file(&path);
        }
    }

    active
}

/// Read the command from a PID file.
fn read_command(pid_file: &Path) -> Option<String> {
    let content = fs::read_to_string(pid_file).ok()?;
    for line in content.lines() {
        if let Some(cmd) = line.strip_prefix("command=") {
            return Some(cmd.to_string());
        }
    }
    None
}

/// Check if a process is still running.
#[cfg(unix)]
fn is_running(pid: u32) -> bool {
    // kill(pid, 0) checks if process exists without sending a signal
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn is_running(_pid: u32) -> bool {
    // On non-Unix platforms, assume the process is running
    // This is a conservative approach that may leave stale entries
    true
}

/// Clean up tracking directory when silo is removed.
pub fn cleanup_tracking(silo_path: &Path) -> Result<(), String> {
    let dir = tracking_dir(silo_path);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Failed to clean up tracking: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_silo_path(suffix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "silo-test-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            suffix
        ));
        path
    }

    #[test]
    fn test_tracking_dir() {
        let silo = PathBuf::from("/var/silo/repo-abc123/feature");
        let tracking = tracking_dir(&silo);
        assert_eq!(
            tracking,
            PathBuf::from("/var/silo/repo-abc123/.tracking/feature")
        );
    }

    #[test]
    fn test_pids_dir() {
        let silo = PathBuf::from("/var/silo/repo-abc123/feature");
        let pids = pids_dir(&silo);
        assert_eq!(
            pids,
            PathBuf::from("/var/silo/repo-abc123/.tracking/feature/pids")
        );
    }

    #[test]
    fn test_register_and_list() {
        let silo = temp_silo_path("register");
        let pid = std::process::id(); // Use our own PID (known to be running)

        // Register
        register(&silo, pid, "test command").unwrap();

        // List should find it
        let active = list_active(&silo);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].pid, pid);
        assert_eq!(active[0].command, "test command");

        // Cleanup
        cleanup_tracking(&silo).unwrap();
    }

    #[test]
    fn test_unregister() {
        let silo = temp_silo_path("unregister");
        let pid = std::process::id();

        register(&silo, pid, "test").unwrap();
        assert_eq!(list_active(&silo).len(), 1);

        unregister(&silo, pid).unwrap();
        assert_eq!(list_active(&silo).len(), 0);

        cleanup_tracking(&silo).unwrap();
    }

    #[test]
    fn test_list_prunes_dead_pids() {
        let silo = temp_silo_path("prune");

        // Create a PID file for a non-existent process
        let dir = pids_dir(&silo);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("999999"), "command=dead\n").unwrap();

        // list_active should prune it
        let active = list_active(&silo);
        assert!(active.is_empty());
        assert!(!dir.join("999999").exists());

        cleanup_tracking(&silo).unwrap();
    }

    #[test]
    fn test_cleanup_tracking() {
        let silo = temp_silo_path("cleanup");
        register(&silo, 12345, "test").unwrap();

        let dir = tracking_dir(&silo);
        assert!(dir.exists());

        cleanup_tracking(&silo).unwrap();
        assert!(!dir.exists());
    }
}
