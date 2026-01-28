use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::warn;

const USER_CONFIG_PATH: &str = ".config/silo.toml";
const LOCAL_CONFIG_NAME: &str = ".silo.toml";
const DEFAULT_WORKTREE_DIR: &str = ".local/var/silo";

/// Known top-level config keys
const KNOWN_KEYS: &[&str] = &[
    "worktree_dir",
    "warn_shell_integration",
    "extra_command_args",
];

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Config {
    pub worktree_dir: Option<String>,
    /// Whether to warn when shell integration is not enabled (default: true)
    pub warn_shell_integration: Option<bool>,
    /// Extra arguments to inject into commands based on command prefix.
    /// Keys are command prefixes (e.g., "git", "git diff"), values are args to insert.
    #[serde(default)]
    pub extra_command_args: HashMap<String, Vec<String>>,
}

impl Config {
    /// Load config with hierarchy: user -> main worktree -> current directory.
    ///
    /// Order (later overrides earlier):
    /// 1. User config (~/.config/silo.toml)
    /// 2. Main worktree config (if in a silo, the original repo's .silo.toml)
    /// 3. Current directory config (.silo.toml)
    pub fn load() -> Result<Self, String> {
        let mut config = Self::load_user()?;

        if let Ok(cwd) = std::env::current_dir() {
            // If we're in a silo worktree, also check the main worktree for config
            if let Some(main_wt) = crate::git::get_main_worktree_from_silo(&cwd) {
                let main_config = Self::load_local(&main_wt)?;
                config = config.merge(main_config);
            }

            // Current directory config has highest priority
            let local_config = Self::load_local(&cwd)?;
            config = config.merge(local_config);
        }

        Ok(config)
    }

    /// Load config exclusively from a specific file (ignores default locations).
    /// Unlike load_from_path, this returns an error if the file doesn't exist.
    pub fn load_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!("Config file not found: {}", path.display()));
        }
        Self::load_from_path(path)
    }

    /// Load user config from ~/.config/silo.toml
    fn load_user() -> Result<Self, String> {
        let home = std::env::var("HOME").map_err(|_| "HOME environment variable not set")?;
        let config_path = PathBuf::from(&home).join(USER_CONFIG_PATH);
        Self::load_from_path(&config_path)
    }

    /// Load local config from path/.silo.toml (returns default if not exists)
    fn load_local(path: &Path) -> Result<Self, String> {
        let config_path = path.join(LOCAL_CONFIG_NAME);
        Self::load_from_path(&config_path)
    }

    /// Load config from a specific path (returns default if not exists)
    fn load_from_path(config_path: &Path) -> Result<Self, String> {
        if !config_path.exists() {
            return Ok(Config::default());
        }

        let contents = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read {}: {}", config_path.display(), e))?;

        // First parse as generic TOML to check for unknown keys
        if let Ok(value) = contents.parse::<toml::Table>() {
            let known: HashSet<&str> = KNOWN_KEYS.iter().copied().collect();
            for key in value.keys() {
                if !known.contains(key.as_str()) {
                    warn!(
                        file = %config_path.display(),
                        key = %key,
                        "Unknown config key (ignored)"
                    );
                }
            }
        }

        toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse {}: {}", config_path.display(), e))
    }

    /// Merge another config into this one (other takes precedence for set values).
    /// For extra_command_args, entries from both configs are combined (not overridden).
    fn merge(self, other: Self) -> Self {
        let mut extra_command_args = self.extra_command_args;
        for (key, args) in other.extra_command_args {
            extra_command_args.entry(key).or_default().extend(args);
        }

        Config {
            worktree_dir: other.worktree_dir.or(self.worktree_dir),
            warn_shell_integration: other.warn_shell_integration.or(self.warn_shell_integration),
            extra_command_args,
        }
    }

    /// Get the extra args configuration for commands.
    pub fn extra_command_args(&self) -> &HashMap<String, Vec<String>> {
        &self.extra_command_args
    }

    /// Whether to warn when shell integration is not enabled (default: true)
    pub fn warn_shell_integration(&self) -> bool {
        self.warn_shell_integration.unwrap_or(true)
    }

    /// Get the worktree directory, expanding ~ to $HOME
    pub fn get_worktree_dir(&self) -> Result<PathBuf, String> {
        let home = std::env::var("HOME").map_err(|_| "HOME environment variable not set")?;

        let path = self.worktree_dir.as_deref().unwrap_or(DEFAULT_WORKTREE_DIR);

        // Expand ~ to home directory
        let expanded = if let Some(suffix) = path.strip_prefix("~/") {
            format!("{}/{}", home, suffix)
        } else if path == "~" {
            home
        } else if path.starts_with('/') {
            path.to_string()
        } else {
            // Relative path treated as relative to home
            format!("{}/{}", home, path)
        };

        Ok(PathBuf::from(expanded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_other_takes_precedence() {
        let base = Config {
            worktree_dir: Some("/base/dir".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let other = Config {
            worktree_dir: Some("/other/dir".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let merged = base.merge(other);
        assert_eq!(merged.worktree_dir, Some("/other/dir".to_string()));
    }

    #[test]
    fn test_merge_preserves_base_when_other_none() {
        let base = Config {
            worktree_dir: Some("/base/dir".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let other = Config {
            worktree_dir: None,
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let merged = base.merge(other);
        assert_eq!(merged.worktree_dir, Some("/base/dir".to_string()));
    }

    #[test]
    fn test_merge_both_none() {
        let base = Config {
            worktree_dir: None,
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let other = Config {
            worktree_dir: None,
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let merged = base.merge(other);
        assert_eq!(merged.worktree_dir, None);
    }

    #[test]
    fn test_get_worktree_dir_absolute_path() {
        let config = Config {
            worktree_dir: Some("/absolute/path/to/silos".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let result = config.get_worktree_dir();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/absolute/path/to/silos"));
    }

    #[test]
    fn test_get_worktree_dir_tilde_expansion() {
        let config = Config {
            worktree_dir: Some("~/my/silos".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let result = config.get_worktree_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        // Should start with home directory, not ~
        assert!(!path.to_string_lossy().starts_with('~'));
        assert!(path.to_string_lossy().ends_with("my/silos"));
    }

    #[test]
    fn test_get_worktree_dir_relative_path() {
        let config = Config {
            worktree_dir: Some("relative/path".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let result = config.get_worktree_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        // Relative paths are treated as relative to home
        assert!(path.to_string_lossy().ends_with("relative/path"));
    }

    #[test]
    fn test_get_worktree_dir_default() {
        let config = Config {
            worktree_dir: None,
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let result = config.get_worktree_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        // Default is .local/var/silo
        assert!(path.to_string_lossy().ends_with(".local/var/silo"));
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.worktree_dir.is_none());
        assert!(config.extra_command_args.is_empty());
    }

    #[test]
    fn test_config_clone() {
        let mut extra_args = HashMap::new();
        extra_args.insert("git".to_string(), vec!["--color".to_string()]);
        let config = Config {
            worktree_dir: Some("/test".to_string()),
            warn_shell_integration: Some(false),
            extra_command_args: extra_args,
        };
        let cloned = config.clone();
        assert_eq!(config.worktree_dir, cloned.worktree_dir);
        assert_eq!(config.warn_shell_integration, cloned.warn_shell_integration);
        assert_eq!(config.extra_command_args, cloned.extra_command_args);
    }

    #[test]
    fn test_config_debug() {
        let config = Config {
            worktree_dir: Some("/test".to_string()),
            warn_shell_integration: None,
            extra_command_args: HashMap::new(),
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("/test"));
    }

    #[test]
    fn test_warn_shell_integration_defaults_to_true() {
        let config = Config::default();
        assert!(config.warn_shell_integration());
    }

    #[test]
    fn test_warn_shell_integration_respects_explicit_value() {
        let config = Config {
            worktree_dir: None,
            warn_shell_integration: Some(false),
            extra_command_args: HashMap::new(),
        };
        assert!(!config.warn_shell_integration());

        let config = Config {
            worktree_dir: None,
            warn_shell_integration: Some(true),
            extra_command_args: HashMap::new(),
        };
        assert!(config.warn_shell_integration());
    }

    #[test]
    fn test_merge_warn_shell_integration() {
        let base = Config {
            worktree_dir: None,
            warn_shell_integration: Some(true),
            extra_command_args: HashMap::new(),
        };
        let other = Config {
            worktree_dir: None,
            warn_shell_integration: Some(false),
            extra_command_args: HashMap::new(),
        };
        let merged = base.merge(other);
        assert_eq!(merged.warn_shell_integration, Some(false));
    }

    #[test]
    fn test_parse_extra_command_args() {
        let toml_str = r#"
[extra_command_args]
git = ["-c", "color.ui=always"]
"git diff" = ["--stat"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.extra_command_args.get("git"),
            Some(&vec!["-c".to_string(), "color.ui=always".to_string()])
        );
        assert_eq!(
            config.extra_command_args.get("git diff"),
            Some(&vec!["--stat".to_string()])
        );
    }

    #[test]
    fn test_merge_extra_command_args_combines() {
        let mut base_args = HashMap::new();
        base_args.insert("git".to_string(), vec!["-c".to_string(), "a=1".to_string()]);
        base_args.insert("cargo".to_string(), vec!["--color=always".to_string()]);

        let mut other_args = HashMap::new();
        other_args.insert("git".to_string(), vec!["-c".to_string(), "b=2".to_string()]);
        other_args.insert("npm".to_string(), vec!["--silent".to_string()]);

        let base = Config {
            worktree_dir: None,
            warn_shell_integration: None,
            extra_command_args: base_args,
        };
        let other = Config {
            worktree_dir: None,
            warn_shell_integration: None,
            extra_command_args: other_args,
        };

        let merged = base.merge(other);

        // git args should be combined
        assert_eq!(
            merged.extra_command_args.get("git"),
            Some(&vec![
                "-c".to_string(),
                "a=1".to_string(),
                "-c".to_string(),
                "b=2".to_string()
            ])
        );
        // cargo should be preserved from base
        assert_eq!(
            merged.extra_command_args.get("cargo"),
            Some(&vec!["--color=always".to_string()])
        );
        // npm should be added from other
        assert_eq!(
            merged.extra_command_args.get("npm"),
            Some(&vec!["--silent".to_string()])
        );
    }
}
