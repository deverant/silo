use std::io::{IsTerminal, Write};

/// Trait for abstracting terminal input, enabling testable prompts.
pub trait PromptInput {
    fn is_terminal(&self) -> bool;
    fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize>;
}

/// Real stdin implementation of PromptInput.
struct StdinInput;

impl PromptInput for StdinInput {
    fn is_terminal(&self) -> bool {
        std::io::stdin().is_terminal()
    }

    fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        std::io::stdin().read_line(buf)
    }
}

/// Ask user for y/n confirmation using the provided input source.
/// Returns true if confirmed, false otherwise.
fn confirm_with_input(message: &str, input: &mut impl PromptInput) -> bool {
    if !input.is_terminal() {
        return false;
    }

    eprint!("{} [y/N] ", message);
    std::io::stderr().flush().ok();

    let mut line = String::new();
    if input.read_line(&mut line).is_err() {
        return false;
    }

    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Ask user for y/n confirmation. Returns true if confirmed.
/// Returns false if stdin is not a tty (safe default for scripts).
pub fn confirm(message: &str) -> bool {
    confirm_with_input(message, &mut StdinInput)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockInput {
        is_tty: bool,
        response: &'static str,
    }

    impl PromptInput for MockInput {
        fn is_terminal(&self) -> bool {
            self.is_tty
        }

        fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
            buf.push_str(self.response);
            Ok(self.response.len())
        }
    }

    fn mock_tty(response: &'static str) -> MockInput {
        MockInput {
            is_tty: true,
            response,
        }
    }

    fn mock_non_tty(response: &'static str) -> MockInput {
        MockInput {
            is_tty: false,
            response,
        }
    }

    #[test]
    fn test_confirm_lowercase_y() {
        assert!(confirm_with_input("Test?", &mut mock_tty("y\n")));
    }

    #[test]
    fn test_confirm_uppercase_y() {
        assert!(confirm_with_input("Test?", &mut mock_tty("Y\n")));
    }

    #[test]
    fn test_confirm_lowercase_yes() {
        assert!(confirm_with_input("Test?", &mut mock_tty("yes\n")));
    }

    #[test]
    fn test_confirm_uppercase_yes() {
        assert!(confirm_with_input("Test?", &mut mock_tty("YES\n")));
    }

    #[test]
    fn test_confirm_mixed_case_yes() {
        assert!(confirm_with_input("Test?", &mut mock_tty("Yes\n")));
        assert!(confirm_with_input("Test?", &mut mock_tty("yEs\n")));
    }

    #[test]
    fn test_confirm_with_whitespace() {
        assert!(confirm_with_input("Test?", &mut mock_tty("  y  \n")));
        assert!(confirm_with_input("Test?", &mut mock_tty("\ty\n")));
        assert!(confirm_with_input("Test?", &mut mock_tty("  yes  \n")));
    }

    #[test]
    fn test_confirm_rejects_no() {
        assert!(!confirm_with_input("Test?", &mut mock_tty("n\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("N\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("no\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("NO\n")));
    }

    #[test]
    fn test_confirm_rejects_empty() {
        assert!(!confirm_with_input("Test?", &mut mock_tty("\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("  \n")));
    }

    #[test]
    fn test_confirm_rejects_other() {
        assert!(!confirm_with_input("Test?", &mut mock_tty("yep\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("yeah\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("sure\n")));
        assert!(!confirm_with_input("Test?", &mut mock_tty("ok\n")));
    }

    #[test]
    fn test_confirm_returns_false_when_not_terminal() {
        assert!(!confirm_with_input("Test?", &mut mock_non_tty("y\n")));
        assert!(!confirm_with_input("Test?", &mut mock_non_tty("yes\n")));
    }
}
