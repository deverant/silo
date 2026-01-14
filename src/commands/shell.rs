//! Shell integration commands: init and complete-args.

use crate::complete;
use crate::shell::{self, ShellType};

/// Output shell integration script.
pub fn init(shell_type: ShellType) -> Result<(), String> {
    // Get the path to the silo binary
    let silo_bin = std::env::current_exe()
        .map_err(|e| format!("Failed to get silo path: {}", e))?
        .display()
        .to_string();

    let script = match shell_type {
        ShellType::Zsh => shell::zsh::init_script(&silo_bin),
    };

    print!("{}", script);
    Ok(())
}

/// Generate completions for any position (for shell completion).
pub fn complete_args(args: &[String]) {
    for c in complete::generate(args) {
        println!("{}", c.format_zsh());
    }
}
