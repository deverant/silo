use std::io::{IsTerminal, Write};

/// Ask user for y/n confirmation. Returns true if confirmed.
/// Returns false if stdin is not a tty (safe default for scripts).
pub fn confirm(message: &str) -> bool {
    let stdin = std::io::stdin();

    if !stdin.is_terminal() {
        return false;
    }

    eprint!("{} [y/N] ", message);
    std::io::stderr().flush().ok();

    let mut input = String::new();
    if stdin.read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    /// Check if user input matches "yes" confirmation.
    /// This is a helper for testing confirm logic without needing stdin.
    fn is_yes(input: &str) -> bool {
        matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
    }

    #[test]
    fn test_is_yes_lowercase_y() {
        assert!(is_yes("y"));
    }

    #[test]
    fn test_is_yes_uppercase_y() {
        assert!(is_yes("Y"));
    }

    #[test]
    fn test_is_yes_lowercase_yes() {
        assert!(is_yes("yes"));
    }

    #[test]
    fn test_is_yes_uppercase_yes() {
        assert!(is_yes("YES"));
    }

    #[test]
    fn test_is_yes_mixed_case_yes() {
        assert!(is_yes("Yes"));
        assert!(is_yes("yEs"));
    }

    #[test]
    fn test_is_yes_with_whitespace() {
        assert!(is_yes("  y  "));
        assert!(is_yes("\ty\n"));
        assert!(is_yes("  yes  \n"));
    }

    #[test]
    fn test_is_yes_rejects_no() {
        assert!(!is_yes("n"));
        assert!(!is_yes("N"));
        assert!(!is_yes("no"));
        assert!(!is_yes("NO"));
    }

    #[test]
    fn test_is_yes_rejects_empty() {
        assert!(!is_yes(""));
        assert!(!is_yes("  "));
    }

    #[test]
    fn test_is_yes_rejects_other() {
        assert!(!is_yes("yep"));
        assert!(!is_yes("yeah"));
        assert!(!is_yes("sure"));
        assert!(!is_yes("ok"));
    }
}
