//! The `list` command: list silos for the current repo or all repos.

use std::path::{Path, PathBuf};

use crate::color;
use crate::git;
use crate::process;
use crate::silo;

/// Stats for a silo, used for sorting and display.
struct SiloDisplayInfo {
    display_name: String,
    branch: String,
    path: PathBuf,
    ahead: u32,
    behind: u32,
    added: u32,
    removed: u32,
    uncommitted: git::UncommittedStats,
    process_count: usize,
    is_current: bool,
}

pub fn run(all: bool, use_color: bool, quiet: bool) -> Result<(), String> {
    // Auto-use --all if we're not in a git repository
    let repo_root = git::try_get_repo_root();
    let list_all = all || repo_root.is_none();

    if list_all {
        run_all(use_color, quiet)
    } else {
        run_repo(&repo_root.unwrap(), use_color, quiet)
    }
}

fn run_repo(repo_root: &Path, use_color: bool, quiet: bool) -> Result<(), String> {
    use std::io::IsTerminal;

    let silos = silo::collect_silos_for_repo(repo_root)?;

    if silos.is_empty() {
        return Ok(());
    }

    // Get main branch from worktrees
    let worktrees = git::list_worktrees(repo_root)?;
    let main_worktree = worktrees.first();
    let main_branch = main_worktree
        .map(|wt| wt.branch_name())
        .unwrap_or("(detached)");

    // Get current working directory to mark current worktree
    let current_dir = std::env::current_dir().ok();

    // Quiet mode: only print silo names (directory names)
    if quiet {
        for s in &silos {
            println!("{}", s.name);
        }
        return Ok(());
    }

    // Collect stats for all silos
    let mut silo_stats: Vec<SiloDisplayInfo> = silos
        .iter()
        .map(|s| {
            let branch = s.branch.as_deref().unwrap_or("(detached)").to_string();
            let is_current = current_dir
                .as_ref()
                .map(|cwd| cwd.starts_with(&s.storage_path))
                .unwrap_or(false);
            let (ahead, behind) = git::get_ahead_behind(&s.storage_path, &branch, main_branch);
            let (added, removed) = git::get_diff_stats(&s.storage_path, &branch, main_branch);
            let uncommitted = git::get_uncommitted_stats(&s.storage_path);
            let process_count = process::list_active(&s.storage_path).len();

            SiloDisplayInfo {
                display_name: s.name.clone(),
                branch,
                path: s.storage_path.clone(),
                ahead,
                behind,
                added,
                removed,
                uncommitted,
                process_count,
                is_current,
            }
        })
        .collect();

    // Sort by ahead count descending (most commits first)
    silo_stats.sort_by(|a, b| b.ahead.cmp(&a.ahead));

    let is_tty = std::io::stdout().is_terminal();
    let is_current_main = current_dir
        .as_ref()
        .zip(main_worktree)
        .map(|(cwd, wt)| cwd.starts_with(&wt.path))
        .unwrap_or(false);

    if is_tty {
        // Calculate column widths
        let name_width = silo_stats
            .iter()
            .map(|s| s.display_name.len())
            .chain(std::iter::once(main_branch.len()))
            .max()
            .unwrap_or(4)
            .max(4); // minimum width for "NAME"

        let branch_width = silo_stats
            .iter()
            .map(|s| s.branch.len())
            .chain(std::iter::once(main_branch.len()))
            .max()
            .unwrap_or(6)
            .max(6); // minimum width for "BRANCH"

        // Print header
        println!(
            "  {:<nw$}  {:<bw$}  {:>12}  {:>14}  UNCOMMITTED",
            "NAME",
            "BRANCH",
            "COMMITS",
            "LINES",
            nw = name_width,
            bw = branch_width
        );

        // Print main worktree
        let marker = if is_current_main { "*" } else { " " };
        println!(
            "{} {:<nw$}  {:<bw$}  (main)",
            marker,
            main_branch,
            main_branch,
            nw = name_width,
            bw = branch_width
        );

        // Print silos with aligned columns
        for silo in &silo_stats {
            let marker = if silo.is_current { "*" } else { " " };
            let commits = format!(
                "{} {}",
                color::green_positive(silo.ahead, use_color),
                color::red_negative(silo.behind, use_color)
            );
            let lines = format!(
                "{} {}",
                color::green_positive(silo.added, use_color),
                color::red_negative(silo.removed, use_color)
            );
            let uncommitted_str =
                format_uncommitted_short(&silo.uncommitted, &silo.path, use_color);
            let process_str = format_process_count(silo.process_count, use_color);
            let suffix = format_suffix(&uncommitted_str, &process_str);

            // Calculate visible widths (without ANSI codes)
            let commits_visible = format!("+{} -{}", silo.ahead, silo.behind);
            let lines_visible = format!("+{} -{}", silo.added, silo.removed);
            let commits_padding = 12_usize.saturating_sub(commits_visible.len());
            let lines_padding = 14_usize.saturating_sub(lines_visible.len());

            println!(
                "{} {:<nw$}  {:<bw$}  {:>cp$}{}  {:>lp$}{}  {}",
                marker,
                silo.display_name,
                silo.branch,
                "",
                commits,
                "",
                lines,
                suffix,
                nw = name_width,
                bw = branch_width,
                cp = commits_padding,
                lp = lines_padding,
            );
        }
    } else {
        // Non-TTY: simple format without headers
        let marker = if is_current_main { "*" } else { " " };
        println!("{} {} ({})  (main)", marker, main_branch, main_branch);

        for silo in &silo_stats {
            let marker = if silo.is_current { "*" } else { " " };
            let uncommitted_str =
                format_uncommitted_with_files(&silo.uncommitted, &silo.path, use_color);
            let process_str = format_process_count(silo.process_count, use_color);

            // Build suffix with proper separators
            // uncommitted_str already has leading ", " when non-empty
            let suffix = match (uncommitted_str.is_empty(), process_str.is_empty()) {
                (true, true) => String::new(),
                (true, false) => format!(", {}", process_str),
                (false, true) => uncommitted_str,
                (false, false) => format!("{}, {}", uncommitted_str, process_str),
            };

            println!(
                "{} {} ({})  {} {} commits, {} {} lines{}",
                marker,
                silo.display_name,
                silo.branch,
                color::green_positive(silo.ahead, use_color),
                color::red_negative(silo.behind, use_color),
                color::green_positive(silo.added, use_color),
                color::red_negative(silo.removed, use_color),
                suffix
            );
        }
    }

    Ok(())
}

fn run_all(use_color: bool, quiet: bool) -> Result<(), String> {
    use std::io::IsTerminal;

    let silos = silo::collect_all_silos()?;

    if silos.is_empty() {
        if !quiet {
            println!("No silos found.");
        }
        return Ok(());
    }

    let current_dir = std::env::current_dir().ok();

    // Quiet mode: only print silo names
    if quiet {
        for s in &silos {
            println!("{}/{}", s.repo_name, s.name);
        }
        return Ok(());
    }

    // Group silos by repo (using main_worktree path as key)
    let mut repos: std::collections::HashMap<PathBuf, Vec<&silo::Silo>> =
        std::collections::HashMap::new();

    for s in &silos {
        repos.entry(s.main_worktree.clone()).or_default().push(s);
    }

    // Sort repos by name (get name from first silo in each group)
    let mut sorted_repos: Vec<_> = repos.into_values().collect();
    sorted_repos.sort_by(|a, b| {
        let name_a = a.first().map(|s| s.repo_name.as_str()).unwrap_or("");
        let name_b = b.first().map(|s| s.repo_name.as_str()).unwrap_or("");
        name_a.cmp(name_b)
    });

    let is_tty = std::io::stdout().is_terminal();

    // Calculate global name and branch widths for TTY mode
    let (global_name_width, global_branch_width) = if is_tty {
        let name_width = sorted_repos
            .iter()
            .flat_map(|repo_silos| {
                let repo_name = repo_silos
                    .first()
                    .map(|s| s.repo_name.as_str())
                    .unwrap_or("");
                let main_name_len = repo_name.len() + 1 + repo_name.len(); // rough estimate
                repo_silos
                    .iter()
                    .map(move |s| repo_name.len() + 1 + s.name.len())
                    .chain(std::iter::once(main_name_len))
            })
            .max()
            .unwrap_or(4)
            .max(4);
        let branch_width = sorted_repos
            .iter()
            .flat_map(|repo_silos| {
                repo_silos
                    .iter()
                    .map(|s| s.branch.as_deref().unwrap_or("(detached)").len())
            })
            .max()
            .unwrap_or(6)
            .max(6);
        (name_width, branch_width)
    } else {
        (0, 0)
    };

    // Print header for TTY
    if is_tty {
        println!(
            "  {:<nw$}  {:<bw$}  {:>12}  {:>14}  UNCOMMITTED",
            "NAME",
            "BRANCH",
            "COMMITS",
            "LINES",
            nw = global_name_width,
            bw = global_branch_width
        );
    }

    let mut first_repo = true;
    for repo_silos in sorted_repos {
        // Add empty line between repositories
        if !first_repo {
            println!();
        }
        first_repo = false;

        // Get repo info from first silo
        let Some(first_silo) = repo_silos.first() else {
            continue;
        };
        let repo_name = &first_silo.repo_name;
        let main_worktree = &first_silo.main_worktree;

        // Get main worktree info
        let worktrees = git::list_worktrees(main_worktree)?;
        let main_wt = worktrees.first();
        let main_branch = main_wt
            .and_then(|wt| wt.branch.as_deref())
            .unwrap_or("main");

        let is_current_main = current_dir
            .as_ref()
            .map(|cwd| cwd.starts_with(main_worktree))
            .unwrap_or(false);

        // Collect stats for all silos in this repo
        let mut silo_stats: Vec<SiloDisplayInfo> = repo_silos
            .iter()
            .map(|s| {
                let display_name = format!("{}/{}", repo_name, s.name);
                let is_current = current_dir
                    .as_ref()
                    .map(|cwd| cwd.starts_with(&s.storage_path))
                    .unwrap_or(false);
                let branch_str = s.branch.as_deref().unwrap_or("(detached)");
                let (ahead, behind) =
                    git::get_ahead_behind(&s.storage_path, branch_str, main_branch);
                let (added, removed) =
                    git::get_diff_stats(&s.storage_path, branch_str, main_branch);
                let uncommitted = git::get_uncommitted_stats(&s.storage_path);
                let process_count = process::list_active(&s.storage_path).len();

                SiloDisplayInfo {
                    display_name,
                    branch: branch_str.to_string(),
                    path: s.storage_path.clone(),
                    ahead,
                    behind,
                    added,
                    removed,
                    uncommitted,
                    process_count,
                    is_current,
                }
            })
            .collect();

        // Sort by ahead count descending (most commits first)
        silo_stats.sort_by(|a, b| b.ahead.cmp(&a.ahead));

        let main_display_name = format!("{}/{}", repo_name, main_branch);

        if is_tty {
            // Print main worktree
            let marker = if is_current_main { "*" } else { " " };
            println!(
                "{} {:<nw$}  {:<bw$}  {}",
                marker,
                main_display_name,
                main_branch,
                main_worktree.display(),
                nw = global_name_width,
                bw = global_branch_width
            );

            // Print silos with aligned columns
            for silo in &silo_stats {
                let marker = if silo.is_current { "*" } else { " " };
                let commits = format!(
                    "{} {}",
                    color::green_positive(silo.ahead, use_color),
                    color::red_negative(silo.behind, use_color)
                );
                let lines = format!(
                    "{} {}",
                    color::green_positive(silo.added, use_color),
                    color::red_negative(silo.removed, use_color)
                );
                let uncommitted_str =
                    format_uncommitted_short(&silo.uncommitted, &silo.path, use_color);
                let process_str = format_process_count(silo.process_count, use_color);
                let suffix = format_suffix(&uncommitted_str, &process_str);

                // Calculate visible widths (without ANSI codes)
                let commits_visible = format!("+{} -{}", silo.ahead, silo.behind);
                let lines_visible = format!("+{} -{}", silo.added, silo.removed);
                let commits_padding = 12_usize.saturating_sub(commits_visible.len());
                let lines_padding = 14_usize.saturating_sub(lines_visible.len());

                println!(
                    "{} {:<nw$}  {:<bw$}  {:>cp$}{}  {:>lp$}{}  {}",
                    marker,
                    silo.display_name,
                    silo.branch,
                    "",
                    commits,
                    "",
                    lines,
                    suffix,
                    nw = global_name_width,
                    bw = global_branch_width,
                    cp = commits_padding,
                    lp = lines_padding,
                );
            }
        } else {
            // Non-TTY: simple format without headers
            let marker = if is_current_main { "*" } else { " " };
            println!(
                "{} {} ({})  {}",
                marker,
                main_display_name,
                main_branch,
                main_worktree.display()
            );

            for silo in &silo_stats {
                let marker = if silo.is_current { "*" } else { " " };
                let uncommitted_str =
                    format_uncommitted_with_files(&silo.uncommitted, &silo.path, use_color);
                let process_str = format_process_count(silo.process_count, use_color);

                // Build suffix with proper separators
                // uncommitted_str already has leading ", " when non-empty
                let suffix = match (uncommitted_str.is_empty(), process_str.is_empty()) {
                    (true, true) => String::new(),
                    (true, false) => format!(", {}", process_str),
                    (false, true) => uncommitted_str,
                    (false, false) => format!("{}, {}", uncommitted_str, process_str),
                };

                println!(
                    "{} {} ({})  {} {} commits, {} {} lines{}",
                    marker,
                    silo.display_name,
                    silo.branch,
                    color::green_positive(silo.ahead, use_color),
                    color::red_negative(silo.behind, use_color),
                    color::green_positive(silo.added, use_color),
                    color::red_negative(silo.removed, use_color),
                    suffix
                );
            }
        }
    }

    Ok(())
}

/// Format uncommitted changes in short form for TTY output.
fn format_uncommitted_short(
    uncommitted: &git::UncommittedStats,
    path: &Path,
    use_color: bool,
) -> String {
    if uncommitted.is_clean() {
        return String::new();
    }

    let files = git::get_uncommitted_files(path);
    let total = uncommitted.total();

    // Limit displayed files to 3, with ellipsis for more
    const MAX_FILES: usize = 3;
    let file_list = if files.len() <= MAX_FILES {
        files.join(", ")
    } else {
        let shown: Vec<_> = files.iter().take(MAX_FILES).cloned().collect();
        format!("{}, +{} more", shown.join(", "), files.len() - MAX_FILES)
    };

    format!(
        "{} files: {}",
        color::yellow_uncommitted(total, use_color),
        file_list
    )
}

/// Format uncommitted changes with file names.
fn format_uncommitted_with_files(
    uncommitted: &git::UncommittedStats,
    path: &Path,
    use_color: bool,
) -> String {
    if uncommitted.is_clean() {
        return String::new();
    }

    let files = git::get_uncommitted_files(path);
    let total = uncommitted.total();

    // Limit displayed files to 3, with ellipsis for more
    const MAX_FILES: usize = 3;
    let file_list = if files.len() <= MAX_FILES {
        files.join(", ")
    } else {
        let shown: Vec<_> = files.iter().take(MAX_FILES).cloned().collect();
        format!(
            "{}, ... and {} more",
            shown.join(", "),
            files.len() - MAX_FILES
        )
    };

    format!(
        ", {} uncommitted files: {}",
        color::yellow_uncommitted(total, use_color),
        file_list
    )
}

/// Format active process count for display.
fn format_process_count(count: usize, use_color: bool) -> String {
    if count == 0 {
        return String::new();
    }

    let label = if count == 1 { "process" } else { "processes" };
    let count_str = if use_color {
        format!("\x1b[1;35m{}\x1b[0m", count) // Bold magenta
    } else {
        count.to_string()
    };

    format!("{} {}", count_str, label)
}

/// Combine uncommitted and process strings with proper separators.
fn format_suffix(uncommitted: &str, process: &str) -> String {
    match (uncommitted.is_empty(), process.is_empty()) {
        (true, true) => String::new(),
        (true, false) => process.to_string(),
        (false, true) => uncommitted.to_string(),
        (false, false) => format!("{}, {}", uncommitted, process),
    }
}
