//! Shell completion generation.
//!
//! Generates completion candidates based on current command-line position.
//! Returns data that can be formatted for any shell.

use crate::{Cli, git, names, silo};
use clap::CommandFactory;

/// A completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub value: String,
    pub description: Option<String>,
}

impl Completion {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: None,
        }
    }

    fn with_desc(value: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: Some(desc.into()),
        }
    }

    /// Format for zsh completion (value:description with escaped colons).
    pub fn format_zsh(&self) -> String {
        match &self.description {
            Some(desc) => format!("{}:{}", self.value, desc.replace(':', "\\:")),
            None => self.value.clone(),
        }
    }
}

/// Generate completions for the given arguments (words after 'silo').
pub fn generate(args: &[String]) -> Vec<Completion> {
    let cli = Cli::command();
    complete_command(&cli, args)
}

/// Walk the command tree recursively to find completions for current position.
fn complete_command(cmd: &clap::Command, args: &[String]) -> Vec<Completion> {
    // No args: complete subcommands at current level
    if args.is_empty() {
        return subcommands(cmd);
    }

    let first = &args[0];

    // Find matching subcommand
    let subcmd = cmd
        .get_subcommands()
        .find(|c| c.get_name() == first || c.get_visible_aliases().any(|a| a == first));

    match subcmd {
        Some(sub) if sub.has_subcommands() => {
            // Has nested subcommands: recurse
            complete_command(sub, &args[1..])
        }
        Some(_) => {
            // Leaf command: check for special argument completion
            complete_leaf(first, &args[1..])
        }
        None => {
            // Unknown/partial: offer subcommands for filtering
            subcommands(cmd)
        }
    }
}

/// Get subcommand completions for a command.
fn subcommands(cmd: &clap::Command) -> Vec<Completion> {
    let mut out = Vec::new();

    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() {
            continue;
        }

        let desc = sub.get_about().map(|s| s.to_string()).unwrap_or_default();
        out.push(Completion::with_desc(sub.get_name(), &desc));

        for alias in sub.get_visible_aliases() {
            out.push(Completion::with_desc(alias, format!("{} (alias)", desc)));
        }
    }

    out
}

/// Complete arguments for leaf commands (no subcommands).
fn complete_leaf(cmd_name: &str, remaining: &[String]) -> Vec<Completion> {
    // Check if we're completing the first positional arg.
    // remaining is empty when there's no partial word yet,
    // or has one element (possibly empty) when user is typing.
    let completing_first_arg = remaining.is_empty() || remaining.len() == 1;

    match cmd_name {
        // Commands that take a silo name as first arg
        "new" | "rm" | "cd" | "exec" | "rebase" | "merge" | "claude" => {
            if completing_first_arg {
                silo_names()
            } else {
                vec![]
            }
        }
        // Commands with no positional args to complete
        _ => vec![],
    }
}

/// Get silo name completions based on current directory context.
fn silo_names() -> Vec<Completion> {
    let repo_root = git::try_get_repo_root();

    if let Some(ref root) = repo_root {
        // Inside a repo: list branches for this repo
        if let Ok(worktrees) = git::list_worktrees(root) {
            return worktrees
                .iter()
                .enumerate()
                .filter_map(|(i, wt)| {
                    let is_main = i == 0;
                    let is_silo = silo::is_silo_path(&wt.path);
                    if (is_main || is_silo) && wt.branch.is_some() {
                        Some(Completion::new(wt.branch.as_ref().unwrap()))
                    } else {
                        None
                    }
                })
                .collect();
        }
    }

    // Not in a repo: list all silos with display names (always include repo prefix)
    if let Ok(silos) = silo::collect_all_silos() {
        let display_names = names::generate_display_names(&silos, true);
        let mut names: Vec<_> = display_names.into_iter().map(Completion::new).collect();
        names.sort_by(|a, b| a.value.cmp(&b.value));
        return names;
    }

    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(completions: &[Completion]) -> Vec<&str> {
        completions.iter().map(|c| c.value.as_str()).collect()
    }

    #[test]
    fn empty_args_returns_top_level_commands() {
        let completions = generate(&[]);
        let v = values(&completions);
        assert!(v.contains(&"new"));
        assert!(v.contains(&"list"));
        assert!(v.contains(&"shell"));
        assert!(v.contains(&"sandbox"));
    }

    #[test]
    fn shell_returns_init() {
        let completions = generate(&["shell".into()]);
        let v = values(&completions);
        assert!(v.contains(&"init"));
        // complete-args is hidden, should not appear
        assert!(!v.contains(&"complete-args"));
    }

    #[test]
    fn shell_init_returns_zsh() {
        let completions = generate(&["shell".into(), "init".into()]);
        let v = values(&completions);
        assert!(v.contains(&"zsh"));
    }

    #[test]
    fn shell_init_zsh_returns_nothing() {
        let completions = generate(&["shell".into(), "init".into(), "zsh".into()]);
        assert!(completions.is_empty(), "got: {:?}", completions);
    }

    #[test]
    fn sandbox_returns_claude() {
        let completions = generate(&["sandbox".into()]);
        let v = values(&completions);
        assert!(v.contains(&"claude"));
    }

    #[test]
    fn sandbox_claude_returns_silos_not_claude() {
        let completions = generate(&["sandbox".into(), "claude".into()]);
        let v = values(&completions);
        // Should not suggest "claude" again
        assert!(!v.contains(&"claude"));
    }

    #[test]
    fn exec_with_empty_arg_returns_silos() {
        // When user types "silo exec " and presses TAB, shell passes ["exec", ""]
        let completions = generate(&["exec".into(), "".into()]);
        // Should return silo names, not empty (this tests the partial word case)
        // We can't assert specific silos exist, but we verify it doesn't return
        // subcommands like it would for a broken implementation
        let v = values(&completions);
        assert!(!v.contains(&"exec"));
        assert!(!v.contains(&"new"));
    }

    #[test]
    fn list_returns_nothing() {
        // No flag completions
        let completions = generate(&["list".into()]);
        assert!(completions.is_empty());
    }

    #[test]
    fn partial_command_returns_commands() {
        // Partial "sh" should still return all commands for shell to filter
        let completions = generate(&["sh".into()]);
        let v = values(&completions);
        assert!(v.contains(&"shell"));
    }

    #[test]
    fn format_zsh_escapes_colons() {
        let c = Completion::with_desc("test", "desc:with:colons");
        assert_eq!(c.format_zsh(), "test:desc\\:with\\:colons");
    }

    #[test]
    fn format_zsh_no_description() {
        let c = Completion::new("branch-name");
        assert_eq!(c.format_zsh(), "branch-name");
    }
}
