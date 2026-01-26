use clap::{
    CommandFactory, FromArgMatches, Parser, Subcommand,
    builder::styling::{AnsiColor, Effects, Styles},
};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

mod color;
mod commands;
mod complete;
mod config;
mod error;
mod exit;
mod git;
mod names;
mod process;
mod prompt;
mod removal;
mod sandbox;
mod shell;
mod silo;

#[derive(Parser)]
#[command(name = "silo", styles = STYLES)]
#[command(about = "Manage isolated git worktrees for parallel development")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show what would be done without making changes
    #[arg(short = 'n', long, global = true)]
    dry_run: bool,

    /// Skip confirmation prompts
    #[arg(short, long, global = true)]
    force: bool,

    /// Suppress non-error output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Enable verbose output (debug-level logging)
    #[arg(short = 'v', long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new silo with a new branch
    New {
        /// Branch name to create
        branch: String,
        /// Command to run in the new silo
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// List silos for the current repo
    #[command(visible_alias = "ls")]
    List {
        /// List silos for all repositories
        #[arg(short, long)]
        all: bool,
    },
    /// Remove a silo
    ///
    /// If the branch has been merged into the main worktree, it will be deleted.
    /// Otherwise, the branch is preserved.
    #[command(after_help = "NAME can be a branch, repo/branch, or org/repo/branch")]
    Rm {
        /// Silo to remove (branch, repo/branch, or org/repo/branch)
        name: String,
    },
    /// Navigate to a silo directory
    #[command(
        after_help = "NAME can be a branch, repo/branch, or org/repo/branch.\n\
        With no arguments, returns to the main worktree."
    )]
    Cd {
        /// Silo to navigate to (branch, repo/branch, or org/repo/branch)
        name: Option<String>,
    },
    /// Run a command in a silo directory
    #[command(
        visible_alias = "run",
        after_help = "NAME can be a branch, repo/branch, or org/repo/branch"
    )]
    Exec {
        /// Silo to run in (branch, repo/branch, or org/repo/branch)
        name: String,
        /// Command and arguments to execute
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Remove silos with no uncommitted changes
    Prune {
        /// Prune silos for all repositories
        #[arg(short, long)]
        all: bool,
    },
    /// Rebase a silo's commits on top of the main branch
    #[command(after_help = "NAME can be a branch, repo/branch, or org/repo/branch")]
    Rebase {
        /// Silo to rebase (branch, repo/branch, or org/repo/branch)
        name: String,
    },
    /// Merge a silo's branch into the main worktree's current branch
    #[command(
        after_help = "NAME can be a branch, repo/branch, or org/repo/branch.\nMust be run from the main worktree."
    )]
    Merge {
        /// Silo to merge (branch, repo/branch, or org/repo/branch)
        name: String,
    },
    /// Reset a silo to the main worktree's current commit
    ///
    /// Discards all changes in the silo and resets it to match the current
    /// HEAD commit of the main worktree. Use --force to skip confirmation
    /// when the silo has uncommitted changes or unmerged commits.
    #[command(after_help = "NAME can be a branch, repo/branch, or org/repo/branch")]
    Reset {
        /// Silo to reset (branch, repo/branch, or org/repo/branch)
        name: String,
    },
    /// Shell integration commands
    Shell {
        #[command(subcommand)]
        command: ShellCommands,
    },
    /// Run a sandboxed agent in a silo
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommands,
    },
    /// Claude Code skill management
    Skill {
        #[command(subcommand)]
        command: SkillCommands,
    },
}

#[derive(Subcommand)]
enum ShellCommands {
    /// Output shell integration script
    Init {
        #[command(subcommand)]
        shell: shell::ShellType,
    },
    /// Generate completions for any position (for shell completion)
    #[command(hide = true)]
    CompleteArgs {
        /// Current command line words (after 'silo')
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SandboxCommands {
    /// Run Claude Code in a Docker sandbox
    #[command(
        after_help = "SILO can be a branch, repo/branch, or org/repo/branch.\nIf not specified, uses current directory if it's a silo."
    )]
    Claude {
        /// Silo to run in (omit to use current directory)
        silo: Option<String>,

        /// Arguments to pass to Claude Code (after --)
        #[arg(last = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SkillCommands {
    /// Install/update the Claude Code skill
    Init {
        /// Install globally (~/.claude/skills/) instead of project-level
        #[arg(short, long)]
        global: bool,
    },
}

fn main() {
    let cli = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&cli).expect("clap argument parsing invariant");

    // Initialize tracing with appropriate filter level
    // RUST_LOG env var takes precedence, otherwise use --verbose flag
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cli.verbose {
            EnvFilter::new("debug")
        } else {
            EnvFilter::new("warn")
        }
    });

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).without_time())
        .with(filter)
        .init();

    let Some(command) = cli.command else {
        // Print help when no command is provided
        Cli::command()
            .print_help()
            .expect("failed to write help to stdout");
        println!();
        return;
    };

    let use_color = color::should_use_color(false);

    let result = match command {
        Commands::New { branch, command } => {
            commands::new::run(branch, &command, cli.dry_run, cli.quiet)
        }
        Commands::List { all } => commands::list::run(all, use_color, cli.quiet),
        Commands::Rm { name } => commands::rm::run(name, cli.dry_run, cli.force, cli.quiet),
        Commands::Cd { name } => commands::cd::run(name),
        Commands::Exec { name, command } => commands::exec::run(name, &command, cli.quiet),
        Commands::Prune { all } => commands::prune::run(all, cli.dry_run, cli.force, cli.quiet),
        Commands::Rebase { name } => commands::rebase::run(name, cli.dry_run, cli.quiet),
        Commands::Merge { name } => commands::merge::run(name, cli.dry_run, cli.quiet),
        Commands::Reset { name } => commands::reset::run(name, cli.dry_run, cli.force, cli.quiet),
        Commands::Shell { command } => match command {
            ShellCommands::Init { shell } => commands::shell::init(shell),
            ShellCommands::CompleteArgs { args } => {
                commands::shell::complete_args(&args);
                Ok(())
            }
        },
        Commands::Sandbox { command } => match command {
            SandboxCommands::Claude { silo, args } => {
                commands::sandbox::claude(silo, cli.dry_run, &args)
            }
        },
        Commands::Skill { command } => match command {
            SkillCommands::Init { global } => commands::skill::init(global, cli.dry_run, cli.quiet),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        // Use specific exit codes for different error types
        let exit_code = match e.as_str() {
            s if s.starts_with("Not in a git repository") => exit::NOT_FOUND,
            s if s.contains("not found") || s.contains("Not found") => exit::NOT_FOUND,
            _ => exit::ERROR,
        };
        std::process::exit(exit_code);
    }
}
