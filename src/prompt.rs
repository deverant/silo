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
