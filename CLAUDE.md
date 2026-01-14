# CLAUDE.md

## Build

```bash
cargo build           # Debug build
cargo build --release # Release build
cargo test            # Run tests
cargo clippy          # Lint
cargo fmt             # Format
```

## Architecture

Silo manages git worktrees for isolated repo editing. Worktrees stored at `~/.local/var/silo/{repo}-{hash}/{branch}` (configurable via `~/.config/silo.toml`). Uses git CLI directly (not libgit2).

**Modules:**

- `main.rs` - CLI entry (clap), command enum definitions
- `commands/` - Command implementations (cd, exec, list, merge, new, prune, rebase, rm, sandbox, shell)
- `git.rs` - Git CLI wrappers (worktree ops, branch status, diff stats)
- `silo.rs` - Silo paths and collection
- `names.rs` - Name resolution (minimal unique display names: branch → repo/branch → org/repo/branch)
- `removal.rs` - Type-safe silo removal with `RemovableSilo` pattern
- `process.rs` - Process tracking for active silo detection
- `config.rs` - Config loading (`~/.config/silo.toml`)
- `shell/` - Shell integration (directive file, zsh wrapper/completions)
- `complete.rs`, `color.rs`, `prompt.rs`, `sandbox.rs` - Utilities

## Commits

**You MUST commit your work. This is non-negotiable.** The user can always go back and change commits afterwards. Never stop work without making sure that there are no uncommitted changes.

### Pre-commit checklist

Run before every commit:
```bash
cargo fmt && cargo clippy && cargo test
```

### Commit rules

1. **Commit IMMEDIATELY after completing each feature/fix** - do not batch
2. Separate tasks = separate commits (never combine unrelated changes)
3. Tests and documentation changes MUST BE in the same commit with the functional change.
4. **Update documentation** (including CLAUDE.md) to reflect architectural or workflow changes.
5. **ALWAYS commit before stopping work - no exceptions**

### Commit message format

```
<title>          # max 50 chars, imperative mood
                 # blank line
<body>           # explain WHY, wrap at 72 chars
```

**Title requirements:**

- **Maximum 50 characters** - this is a hard limit, not a suggestion
- Use imperative mood ("Add feature" not "Added feature")
- No period at the end
- Capitalize first letter

**Before writing a commit message, verify the title length:**
```bash
echo -n "Your commit title here" | wc -c
```

## Code Patterns

### No dead code
Do not use `#[allow(dead_code)]` anywhere.

### Error Handling

All public functions return `Result<T, String>`. Use `?` for propagation.

Format errors as `"Context: details"`:
```rust
.map_err(|e| format!("Failed to create worktree: {}", e))?
```

### Git Operations

Use `git_command()` helper to set working directory:
```rust
fn git_command(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root);
    cmd
}
```

Use `run_git()` for operations that need stdout. Git failures include stderr in error message.

### Testing

- Unit tests in each module under `#[cfg(test)]`
- Integration tests for all CLI commands in `tests/integration.rs`
- All functionality must be tested. Use test-driven development when implementing new functionality.
- Run with `cargo test`

## Common Tasks

### Add a new command

1. Add variant to `Commands` enum in `main.rs`
2. Create `src/commands/{cmd}.rs` with `pub fn run(...) -> Result<(), String>`
3. Add `pub mod {cmd};` to `commands/mod.rs`
4. Add match arm in `main()` dispatch
5. Update `complete.rs` if command takes a silo name argument

### Add shell support

1. Create `src/shell/{shell}.rs` with `init_script()` function
2. Add `pub mod {shell};` to `shell/mod.rs`
3. Add variant to `ShellType` enum in `shell/mod.rs`
4. Add match arm in `commands/shell.rs`
