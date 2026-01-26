# silo

Manage isolated git worktrees for parallel development.

## Overview

Silo creates separate working copies of your repository using git worktrees, each on its own branch. Useful for running multiple AI coding agents on different features simultaneously.

Key features:
- Create isolated worktrees with a single command
- Centralized storage at `~/.local/var/silo/{repo}/{branch}`
- Cross-repo silo management with smart name resolution
- Automatic cleanup of worktrees with no uncommitted changes

## Installation

```bash
cargo install --path .
```

Or install directly from the repository:

```bash
cargo install --git https://github.com/deverant/silo
```

## Usage

```bash
# Create a new silo
silo new feature-branch

# List silos
silo list            # current repo
silo list --all      # all repos

# Navigate to a silo
silo cd feature      # by branch name
silo cd repoA/feature   # disambiguate with repo name

# Run a command in a silo
silo exec feature cargo build

# Remove a silo (preserves the branch)
silo rm feature

# Remove all clean silos
silo prune           # current repo
silo prune --all     # all repos
```

## Name Resolution

Commands like `cd`, `rm`, and `exec` accept flexible silo names:

- `feature` — branch name (works when unambiguous)
- `repoA/feature` — repo/branch (when same branch in multiple repos)
- `org/repo/feature` — full path (when same repo name in different locations)

If a name is ambiguous, silo shows matching options.

## Workflow

1. Create a silo with `silo new branch-name`
2. Work in the isolated worktree (or start an AI agent there)
3. Commit your changes
4. Remove the silo when done — the branch is preserved
5. Merge the branch when ready

## Claude Code Integration

Install the silo skill to let Claude use isolated worktrees:

```bash
# Project-level (current repo only)
silo skill init

# Global (all projects)
silo skill init --global
```
