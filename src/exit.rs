//! Exit code constants for consistent CLI behavior.
//!
//! These codes follow common Unix conventions and provide semantic meaning
//! for different failure modes.

/// General error
pub const ERROR: i32 = 1;

/// Resource not found (silo, repository, etc.)
pub const NOT_FOUND: i32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_nonzero() {
        assert_ne!(ERROR, 0);
    }

    #[test]
    fn test_not_found_is_nonzero() {
        assert_ne!(NOT_FOUND, 0);
    }

    #[test]
    fn test_error_and_not_found_are_different() {
        assert_ne!(ERROR, NOT_FOUND);
    }
}
