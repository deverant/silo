//! Command execution with extra argument injection.

use std::collections::HashMap;
use std::path::Path;

use tracing::debug;

use crate::config::Config;
use crate::process;

/// Run a command in a specific directory with config-based argument injection.
///
/// Applies extra arguments from config based on command prefix matching,
/// then executes the command. The process is tracked while running.
/// Exits the process if the command fails.
pub fn run_command(command: &[String], dir: &Path, config: &Config) -> Result<(), String> {
    let command = apply_extra_args(command, config.extra_command_args());
    run_command_in_dir(&command, dir)
}

/// Run a command in a specific directory, inheriting stdin/stdout/stderr.
/// Tracks the process while running so other commands can see it.
/// Exits the process if the command fails.
fn run_command_in_dir(command: &[String], dir: &Path) -> Result<(), String> {
    let (cmd, args) = command.split_first().ok_or("No command specified")?;

    let command_str = command.join(" ");
    debug!(command = %command_str, dir = %dir.display(), "Running command");

    let mut child = std::process::Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let pid = child.id();

    // Register the process for tracking
    if let Err(e) = process::register(dir, pid, &command_str) {
        eprintln!("Warning: Failed to register process: {}", e);
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for command: {}", e))?;

    // Unregister the process
    if let Err(e) = process::unregister(dir, pid) {
        eprintln!("Warning: Failed to unregister process: {}", e);
    }

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// Apply extra arguments to a command based on matching prefixes.
///
/// For each prefix in `extra_args` that matches the beginning of `command`,
/// the corresponding args are inserted immediately after the match point.
/// Shorter prefixes are processed first, so their args appear earlier.
///
/// Example:
/// ```
/// command = ["git", "diff", "file.txt"]
/// extra_args = {"git" => ["-c", "a=1"], "git diff" => ["--stat"]}
/// result = ["git", "-c", "a=1", "diff", "--stat", "file.txt"]
/// ```
fn apply_extra_args(command: &[String], extra_args: &HashMap<String, Vec<String>>) -> Vec<String> {
    use tracing::trace;
    trace!(?extra_args, "Extra args from config");

    if command.is_empty() || extra_args.is_empty() {
        return command.to_vec();
    }

    // Collect matching prefixes with their word counts
    let mut matches: Vec<(usize, &Vec<String>)> = extra_args
        .iter()
        .filter_map(|(prefix, args)| {
            let prefix_words: Vec<&str> = prefix.split_whitespace().collect();
            if prefix_words.is_empty() {
                return None;
            }
            // Check if command starts with this prefix
            if command.len() >= prefix_words.len()
                && prefix_words
                    .iter()
                    .zip(command.iter())
                    .all(|(p, c)| *p == c)
            {
                Some((prefix_words.len(), args))
            } else {
                None
            }
        })
        .collect();

    if matches.is_empty() {
        trace!("No prefix matches found");
        return command.to_vec();
    }

    trace!(match_count = matches.len(), "Found prefix matches");

    // Sort by prefix length (shortest first)
    matches.sort_by_key(|(len, _)| *len);

    // Build result by inserting args at appropriate positions
    let mut result = Vec::new();
    let mut cmd_idx = 0;

    for (prefix_len, args) in &matches {
        // Add command words up to this prefix length
        while cmd_idx < *prefix_len {
            result.push(command[cmd_idx].clone());
            cmd_idx += 1;
        }
        // Insert the extra args
        result.extend(args.iter().cloned());
    }

    // Add remaining command words
    while cmd_idx < command.len() {
        result.push(command[cmd_idx].clone());
        cmd_idx += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_extra_args_no_match() {
        let command = vec!["cargo".to_string(), "build".to_string()];
        let mut extra_args = HashMap::new();
        extra_args.insert("git".to_string(), vec!["--color".to_string()]);

        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(result, vec!["cargo", "build"]);
    }

    #[test]
    fn test_apply_extra_args_empty_command() {
        let command: Vec<String> = vec![];
        let mut extra_args = HashMap::new();
        extra_args.insert("git".to_string(), vec!["--color".to_string()]);

        let result = apply_extra_args(&command, &extra_args);
        assert!(result.is_empty());
    }

    #[test]
    fn test_apply_extra_args_empty_config() {
        let command = vec!["git".to_string(), "status".to_string()];
        let extra_args = HashMap::new();

        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(result, vec!["git", "status"]);
    }

    #[test]
    fn test_apply_extra_args_single_prefix_match() {
        let command = vec!["git".to_string(), "status".to_string()];
        let mut extra_args = HashMap::new();
        extra_args.insert(
            "git".to_string(),
            vec!["-c".to_string(), "color.ui=always".to_string()],
        );

        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(result, vec!["git", "-c", "color.ui=always", "status"]);
    }

    #[test]
    fn test_apply_extra_args_multi_word_prefix_match() {
        let command = vec![
            "git".to_string(),
            "diff".to_string(),
            "file.txt".to_string(),
        ];
        let mut extra_args = HashMap::new();
        extra_args.insert("git diff".to_string(), vec!["--stat".to_string()]);

        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(result, vec!["git", "diff", "--stat", "file.txt"]);
    }

    #[test]
    fn test_apply_extra_args_both_prefixes_match() {
        let command = vec![
            "git".to_string(),
            "diff".to_string(),
            "file.txt".to_string(),
        ];
        let mut extra_args = HashMap::new();
        extra_args.insert(
            "git".to_string(),
            vec!["-c".to_string(), "color.ui=always".to_string()],
        );
        extra_args.insert("git diff".to_string(), vec!["--stat".to_string()]);

        let result = apply_extra_args(&command, &extra_args);
        // Shorter prefix first, so git args come before diff args
        assert_eq!(
            result,
            vec!["git", "-c", "color.ui=always", "diff", "--stat", "file.txt"]
        );
    }

    #[test]
    fn test_apply_extra_args_only_longer_prefix_matches() {
        let command = vec!["git".to_string(), "status".to_string()];
        let mut extra_args = HashMap::new();
        extra_args.insert("git diff".to_string(), vec!["--stat".to_string()]);

        // "git diff" doesn't match "git status"
        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(result, vec!["git", "status"]);
    }

    #[test]
    fn test_apply_extra_args_command_shorter_than_prefix() {
        let command = vec!["git".to_string()];
        let mut extra_args = HashMap::new();
        extra_args.insert("git diff".to_string(), vec!["--stat".to_string()]);

        // Command is too short to match "git diff"
        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(result, vec!["git"]);
    }

    #[test]
    fn test_apply_extra_args_preserves_remaining_args() {
        let command = vec![
            "git".to_string(),
            "diff".to_string(),
            "--cached".to_string(),
            "file.txt".to_string(),
        ];
        let mut extra_args = HashMap::new();
        extra_args.insert("git".to_string(), vec!["--no-pager".to_string()]);

        let result = apply_extra_args(&command, &extra_args);
        assert_eq!(
            result,
            vec!["git", "--no-pager", "diff", "--cached", "file.txt"]
        );
    }
}
