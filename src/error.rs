//! Error types for the silo CLI.
//!
//! This module provides a unified error type for all silo operations,
//! replacing the scattered `Result<T, String>` pattern with proper error types.

use thiserror::Error;

/// Main error type for silo operations.
#[derive(Error, Debug)]
pub enum SiloError {
    /// Not currently in a git repository
    #[error("Not in a git repository")]
    NotInRepo,

    /// A git command failed
    #[error("Git command failed: {0}")]
    Git(String),

    /// The requested silo was not found
    #[error("Silo not found: {0}")]
    NotFound(String),

    /// The silo name is ambiguous (matches multiple silos)
    #[error("Ambiguous silo name '{name}': matches {matches:?}")]
    Ambiguous { name: String, matches: Vec<String> },

    /// Configuration error
    #[error("Config error: {0}")]
    Config(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Command execution failed
    #[error("Command failed: {0}")]
    Command(String),

    /// User aborted the operation
    #[error("Aborted")]
    Aborted,

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}

/// Convenience type alias for Results using SiloError.
pub type Result<T> = std::result::Result<T, SiloError>;

impl From<String> for SiloError {
    fn from(s: String) -> Self {
        SiloError::Other(s)
    }
}

impl From<&str> for SiloError {
    fn from(s: &str) -> Self {
        SiloError::Other(s.to_string())
    }
}

impl From<SiloError> for String {
    fn from(e: SiloError) -> Self {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_in_repo_display() {
        let err = SiloError::NotInRepo;
        assert_eq!(format!("{}", err), "Not in a git repository");
    }

    #[test]
    fn test_git_error_display() {
        let err = SiloError::Git("failed to checkout".to_string());
        assert_eq!(format!("{}", err), "Git command failed: failed to checkout");
    }

    #[test]
    fn test_not_found_display() {
        let err = SiloError::NotFound("my-feature".to_string());
        assert_eq!(format!("{}", err), "Silo not found: my-feature");
    }

    #[test]
    fn test_ambiguous_display() {
        let err = SiloError::Ambiguous {
            name: "feature".to_string(),
            matches: vec!["repoA/feature".to_string(), "repoB/feature".to_string()],
        };
        let display = format!("{}", err);
        assert!(display.contains("Ambiguous"));
        assert!(display.contains("feature"));
    }

    #[test]
    fn test_io_error_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let silo_err: SiloError = io_err.into();
        assert!(matches!(silo_err, SiloError::Io(_)));
    }

    #[test]
    fn test_string_conversion() {
        let err: SiloError = "something went wrong".into();
        assert!(matches!(err, SiloError::Other(_)));
        assert_eq!(format!("{}", err), "something went wrong");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SiloError>();
    }
}
