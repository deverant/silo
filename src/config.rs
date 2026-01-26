use serde::Deserialize;
use std::path::{Path, PathBuf};

const USER_CONFIG_PATH: &str = ".config/silo.toml";
const LOCAL_CONFIG_NAME: &str = ".silo.toml";
const DEFAULT_WORKTREE_DIR: &str = ".local/var/silo";

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Config {
    pub worktree_dir: Option<String>,
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
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        toml::from_str(&contents).map_err(|e| format!("Failed to parse config file: {}", e))
    }

    /// Merge another config into this one (other takes precedence for set values)
    fn merge(self, other: Self) -> Self {
        Config {
            worktree_dir: other.worktree_dir.or(self.worktree_dir),
        }
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
        };
        let other = Config {
            worktree_dir: Some("/other/dir".to_string()),
        };
        let merged = base.merge(other);
        assert_eq!(merged.worktree_dir, Some("/other/dir".to_string()));
    }

    #[test]
    fn test_merge_preserves_base_when_other_none() {
        let base = Config {
            worktree_dir: Some("/base/dir".to_string()),
        };
        let other = Config { worktree_dir: None };
        let merged = base.merge(other);
        assert_eq!(merged.worktree_dir, Some("/base/dir".to_string()));
    }

    #[test]
    fn test_merge_both_none() {
        let base = Config { worktree_dir: None };
        let other = Config { worktree_dir: None };
        let merged = base.merge(other);
        assert_eq!(merged.worktree_dir, None);
    }

    #[test]
    fn test_get_worktree_dir_absolute_path() {
        let config = Config {
            worktree_dir: Some("/absolute/path/to/silos".to_string()),
        };
        let result = config.get_worktree_dir();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/absolute/path/to/silos"));
    }

    #[test]
    fn test_get_worktree_dir_tilde_expansion() {
        let config = Config {
            worktree_dir: Some("~/my/silos".to_string()),
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
        };
        let result = config.get_worktree_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        // Relative paths are treated as relative to home
        assert!(path.to_string_lossy().ends_with("relative/path"));
    }

    #[test]
    fn test_get_worktree_dir_default() {
        let config = Config { worktree_dir: None };
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
    }

    #[test]
    fn test_config_clone() {
        let config = Config {
            worktree_dir: Some("/test".to_string()),
        };
        let cloned = config.clone();
        assert_eq!(config.worktree_dir, cloned.worktree_dir);
    }

    #[test]
    fn test_config_debug() {
        let config = Config {
            worktree_dir: Some("/test".to_string()),
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("/test"));
    }
}
