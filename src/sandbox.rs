//! Sandbox configuration for running agents in isolated Docker containers.

use crate::process;
use std::path::Path;
use std::process::{Command, Stdio};

/// Configuration for running an agent in a Docker sandbox.
pub struct DockerSandboxConfig {
    /// The runner/agent name (e.g., "claude")
    pub runner: String,
    /// The workspace directory to mount
    pub workspace: std::path::PathBuf,
    /// Credentials mode (e.g., "none" to use mounted settings)
    pub credentials_mode: String,
    /// Volume mounts: (host_path, container_path)
    pub mounts: Vec<(String, String)>,
    /// Additional arguments to pass to the agent
    pub args: Vec<String>,
}

impl DockerSandboxConfig {
    /// Create a configuration for running Claude Code in a Docker sandbox.
    ///
    /// Uses `--credentials=none` to bypass Docker's credential volume,
    /// instead mounting the user's local Claude settings and git config.
    pub fn claude(workspace: &Path, args: Vec<String>) -> Self {
        let home = std::env::var("HOME").unwrap_or_default();

        Self {
            runner: "claude".to_string(),
            workspace: workspace.to_path_buf(),
            credentials_mode: "none".to_string(),
            mounts: vec![
                (
                    format!("{}/.gitconfig", home),
                    "/home/agent/.gitconfig".to_string(),
                ),
                (
                    format!("{}/.claude/settings.json", home),
                    "/home/agent/.claude/settings.json".to_string(),
                ),
                // Mount Google Cloud credentials for Vertex AI
                (
                    format!(
                        "{}/.config/gcloud/application_default_credentials.json",
                        home
                    ),
                    "/home/agent/.config/gcloud/application_default_credentials.json".to_string(),
                ),
            ],
            args,
        }
    }

    /// Convert the configuration to a docker command as a vector of strings.
    pub fn to_command(&self) -> Vec<String> {
        let mut cmd = vec![
            "docker".to_string(),
            "sandbox".to_string(),
            "run".to_string(),
            format!("--credentials={}", self.credentials_mode),
            "-w".to_string(),
            self.workspace.display().to_string(),
        ];

        for (host, container) in &self.mounts {
            if Path::new(host).exists() {
                cmd.push("-v".to_string());
                cmd.push(format!("{}:{}:ro", host, container));
            }
        }

        cmd.push(self.runner.clone());
        cmd.extend(self.args.clone());

        cmd
    }

    /// Print the docker command to stdout (for --dry-run).
    pub fn print(&self) {
        println!("{}", self.to_command().join(" "));
    }

    /// Execute the docker sandbox command.
    /// Tracks the process while running so other commands can see it.
    pub fn run(&self, silo_path: &Path) -> Result<(), String> {
        let cmd_parts = self.to_command();
        let (program, args) = cmd_parts.split_first().ok_or("Empty command")?;

        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to run docker: {}", e))?;

        let pid = child.id();
        let command_str = cmd_parts.join(" ");

        // Register the process for tracking
        if let Err(e) = process::register(silo_path, pid, &command_str) {
            eprintln!("Warning: Failed to register process: {}", e);
        }

        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for docker: {}", e))?;

        // Unregister the process
        if let Err(e) = process::unregister(silo_path, pid) {
            eprintln!("Warning: Failed to unregister process: {}", e);
        }

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_claude_config_basic() {
        let workspace = PathBuf::from("/test/workspace");
        let config = DockerSandboxConfig::claude(&workspace, vec![]);

        assert_eq!(config.runner, "claude");
        assert_eq!(config.workspace, workspace);
        assert_eq!(config.credentials_mode, "none");
        assert_eq!(config.mounts.len(), 3);
    }

    #[test]
    fn test_claude_config_with_args() {
        let workspace = PathBuf::from("/test/workspace");
        let args = vec!["-c".to_string(), "--verbose".to_string()];
        let config = DockerSandboxConfig::claude(&workspace, args.clone());

        assert_eq!(config.args, args);
    }

    #[test]
    fn test_to_command_basic_structure() {
        let workspace = PathBuf::from("/test/workspace");
        let config = DockerSandboxConfig {
            runner: "claude".to_string(),
            workspace: workspace.clone(),
            credentials_mode: "none".to_string(),
            mounts: vec![], // Empty mounts for predictable test
            args: vec![],
        };

        let cmd = config.to_command();
        assert_eq!(cmd[0], "docker");
        assert_eq!(cmd[1], "sandbox");
        assert_eq!(cmd[2], "run");
        assert_eq!(cmd[3], "--credentials=none");
        assert_eq!(cmd[4], "-w");
        assert_eq!(cmd[5], "/test/workspace");
        assert_eq!(cmd[6], "claude");
    }

    #[test]
    fn test_to_command_with_args() {
        let workspace = PathBuf::from("/test/workspace");
        let config = DockerSandboxConfig {
            runner: "claude".to_string(),
            workspace,
            credentials_mode: "none".to_string(),
            mounts: vec![],
            args: vec!["-c".to_string(), "hello".to_string()],
        };

        let cmd = config.to_command();
        assert!(cmd.contains(&"-c".to_string()));
        assert!(cmd.contains(&"hello".to_string()));
    }
}
