use clap::Subcommand;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use crate::config::Config;

pub mod zsh;

/// Supported shell types for integration
#[derive(Subcommand)]
pub enum ShellType {
    /// Zsh shell integration
    Zsh,
}

/// Environment variable for the directive file path
pub const DIRECTIVE_FILE_ENV: &str = "SILO_DIRECTIVE_FILE";

/// Environment variable for the last used silo (previous location for `cd -`)
pub const LAST_ENV: &str = "SILO_LAST";

/// Write a directive to the specified path (if provided).
/// Directives are written as `key=value\n` lines.
fn write_directive_to_path(path: Option<PathBuf>, key: &str, value: &str) {
    let Some(path) = path else {
        return;
    };

    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };

    // Silently ignore write errors - directive file is optional
    let _ = writeln!(file, "{}={}", key, value);
}

/// Check if shell integration is enabled.
pub fn is_enabled() -> bool {
    std::env::var_os(DIRECTIVE_FILE_ENV).is_some()
}

/// Warn the user if shell integration is not enabled.
/// Respects the `warn_shell_integration` config option.
/// Prints to stderr so it doesn't interfere with command output.
pub fn warn_if_not_enabled(config: &Config) {
    if is_enabled() || !config.warn_shell_integration() {
        return;
    }

    eprintln!(
        "\
hint: Shell integration is not enabled. The `cd` command printed the
      path but could not change your shell's working directory.

      To enable shell integration, add to your shell config:

        # For zsh (~/.zshrc):
        eval \"$(silo shell init zsh)\"

      To disable this warning, add to ~/.config/silo.toml:

        warn_shell_integration = false
"
    );
}

/// Write a directive to the directive file (if configured).
/// Directives are written as `key=value\n` lines.
/// If SILO_DIRECTIVE_FILE is not set, this is a no-op.
pub fn write_directive(key: &str, value: &str) {
    let path = std::env::var_os(DIRECTIVE_FILE_ENV).map(PathBuf::from);
    write_directive_to_path(path, key, value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_file(suffix: &str) -> PathBuf {
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
    fn test_write_directive_format() {
        let path = temp_file("format");
        write_directive_to_path(Some(path.clone()), "cd", "/some/path");

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "cd=/some/path\n");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_directive_appends() {
        let path = temp_file("appends");
        write_directive_to_path(Some(path.clone()), "cd", "/path/one");
        write_directive_to_path(Some(path.clone()), "last", "feature-branch");

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "cd=/path/one\nlast=feature-branch\n");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_directive_noop_when_path_none() {
        // Should not panic when path is None
        write_directive_to_path(None, "cd", "/some/path");
    }
}
