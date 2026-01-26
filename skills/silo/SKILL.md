---
name: silo
description: Create isolated git worktrees for safe experimentation. Use when making significant changes, testing risky modifications, or working on features that shouldn't affect the main worktree.
allowed-tools: Bash(silo:*)
---

# Silo - Isolated Worktree Management

When making significant changes to a codebase, create an isolated silo first.
This prevents accidental changes to the main worktree and allows safe experimentation.

## When to create a silo

- Before implementing a new feature
- Before making risky or experimental changes
- When the user asks to work "in isolation" or "on a branch"
- When testing changes that might break things

## Commands

### Create a new silo
```bash
silo new <branch-name>
```

Creates a new worktree at ~/.local/var/silo/{repo}/{branch} with a new branch.

### Work in the silo
After creating, cd to the silo path shown in output, then make your changes.

### List silos
```bash
silo list        # current repo
silo list --all  # all repos
```

### Remove when done
```bash
silo rm <branch-name>
```

The branch is preserved for merging later.

## Workflow

1. Create silo: `silo new feature-xyz`
2. Navigate to it (use the path from output)
3. Make all changes in the silo
4. Commit your work
5. Return to main worktree when done
