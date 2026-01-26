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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_green_positive_without_color() {
        assert_eq!(green_positive(5, false), "+5");
    }

    #[test]
    fn test_green_positive_with_color() {
        let result = green_positive(5, true);
        assert!(result.contains("+5"));
        assert!(result.contains(GREEN));
        assert!(result.contains(RESET));
    }

    #[test]
    fn test_red_negative_without_color() {
        assert_eq!(red_negative(3, false), "-3");
    }

    #[test]
    fn test_red_negative_with_color() {
        let result = red_negative(3, true);
        assert!(result.contains("-3"));
        assert!(result.contains(RED));
        assert!(result.contains(RESET));
    }

    #[test]
    fn test_yellow_uncommitted_without_color() {
        assert_eq!(yellow_uncommitted(7, false), "~7");
    }

    #[test]
    fn test_yellow_uncommitted_with_color() {
        let result = yellow_uncommitted(7, true);
        assert!(result.contains("~7"));
        assert!(result.contains(YELLOW));
        assert!(result.contains(RESET));
    }

    #[test]
    fn test_format_with_color_zero_value() {
        assert_eq!(green_positive(0, false), "+0");
        assert_eq!(red_negative(0, false), "-0");
        assert_eq!(yellow_uncommitted(0, false), "~0");
    }

    #[test]
    fn test_force_color_returns_true() {
        // When force_color is true, should return true regardless of terminal
        assert!(should_use_color(true));
    }
}
