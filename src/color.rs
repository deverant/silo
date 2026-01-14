//! Terminal color formatting utilities.

use std::io::IsTerminal;

/// ANSI color codes
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Check if colors should be used based on terminal support and force flag.
/// Respects the NO_COLOR environment variable (https://no-color.org/).
pub fn should_use_color(force_color: bool) -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    force_color || std::io::stdout().is_terminal()
}

/// Format a value with an optional color and prefix.
fn format_with_color(prefix: char, value: u32, color: &str, use_color: bool) -> String {
    if use_color {
        format!("{}{}{}{}", color, prefix, value, RESET)
    } else {
        format!("{}{}", prefix, value)
    }
}

/// Format a positive number with green color (e.g., "+5")
pub fn green_positive(value: u32, use_color: bool) -> String {
    format_with_color('+', value, GREEN, use_color)
}

/// Format a negative number with red color (e.g., "-3")
pub fn red_negative(value: u32, use_color: bool) -> String {
    format_with_color('-', value, RED, use_color)
}

/// Format uncommitted changes with yellow color (e.g., "~3")
pub fn yellow_uncommitted(value: u32, use_color: bool) -> String {
    format_with_color('~', value, YELLOW, use_color)
}
