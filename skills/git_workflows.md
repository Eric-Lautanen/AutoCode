---
name: git-workflows
description: Use for any git operation - committing, branching, merging, rebasing, resolving conflicts, reading history, or understanding what changed. Load when a task involves version control operations or when you need to understand the state of the repo before making changes.
---

# Git Workflows

## Overview

Git is the source of truth for your project's history. Used well, it gives you confidence to make changes (you can always go back). Used poorly, it creates confusion, lost work, and merge nightmares. The core principle: **commit often, with clear messages, on the right branch.** Small, well-described commits are the foundation of every other git workflow.

## Reading State

Before making any changes, understand the current state:

```bash
git status                    # What's changed? Staged? Unstaged? Untracked?
git diff                      # Unstaged changes (what you've edited but not staged)
git diff --staged             # Staged changes (what will be in the next commit)
git log --oneline -20        # Recent commit history
git log --oneline --graph     # Branch history with merge topology
git branch -a                 # All branches (local and remote)
git remote -v                 # Remote repositories
git stash list                # Saved work in progress
```

**Understand before acting.** Always check `git status` and `git diff` before committing, merging, or rebasing.

## Committing

### Staging Selectively
```bash
git add <file>                # Stage a specific file
git add -p <file>            # Stage interactively — review each hunk
git add -A                   # Stage everything (use with caution)
git restore --staged <file>  # Unstage a file
```

**Rule:** Stage related changes together. Don't mix a bug fix with a refactoring in the same commit.

### Writing Useful Commit Messages

Format: `<type>: <what changed> — <why>`

```
fix: handle null user in order processor — prevents crash when session expires
feat: add email verification to registration — required for compliance
refactor: extract price calculation from order handler — improves testability
chore: upgrade Django to 4.2 — security patch for CVE-2024-12345
```

**What makes a good commit message:**
- The first line says **what** changed and **why** (not how)
- It's specific enough that `git log --oneline` is useful
- It doesn't assume the reader has context — future you won't remember

**Bad commit messages:**
- `fix` — fix what?
- `wip` — what work?
- `updates` — updates to what?
- `asdfasdf` — no.

## Branching

### Creating and Switching
```bash
git checkout -b feature/email-verification   # Create and switch
git switch -c feature/email-verification     # Same thing (newer syntax)
git checkout main                             # Switch back to main
```

### Tracking Remote Branches
```bash
git push -u origin feature/email-verification  # Push and set upstream
git checkout --track origin/feature/email-verification  # Track existing remote branch
```

### Branch Naming Conventions
- `feature/<description>` — new features
- `fix/<description>` — bug fixes
- `refactor/<description>` — code cleanup
- `chore/<description>` — maintenance tasks

## Merging vs. Rebasing

### When to Merge
- You want to preserve the complete history
- Multiple people worked on the branch
- You're merging a long-lived branch

```bash
git checkout main
git merge feature/email-verification
# Creates a merge commit
```

### When to Rebase
- You want a clean, linear history
- You're updating a feature branch with latest main changes
- You're the only one working on the branch

```bash
git checkout feature/email-verification
git rebase main
# Rewrites your branch commits on top of main
```

**Never rebase shared branches.** If others have pulled your branch, rebasing rewrites history and creates divergent states.

## Conflict Resolution

### Reading Conflict Markers
```
<<<<<<< HEAD
your current changes
=======
incoming changes from the other branch
>>>>>>> feature/email-verification
```

### Resolution Strategy
1. **Read both sides.** Understand what each change was trying to do.
2. **Choose or combine.** Sometimes you want one side, sometimes both, sometimes a new solution.
3. **Remove the markers.** Delete `<<<<<<<`, `=======`, `>>>>>>>` lines.
4. **Test.** Build and run after resolving — conflicts often introduce subtle bugs.

**Common patterns:**
- Both sides added imports: combine them
- Both sides modified the same function: understand the intent of each change and write code that satisfies both
- One side deleted, other modified: usually you want to keep the modification (or delete if the deletion was intentional)

## Undoing Things

| What you want | Command | Safety |
|---------------|---------|--------|
| Unstage a file | `git restore --staged <file>` | Safe — doesn't change file content |
| Discard working changes | `git restore <file>` | **Destructive** — loses uncommitted changes |
| Undo last commit (keep changes) | `git reset --soft HEAD~1` | Safe — changes stay staged |
| Undo last commit (unstage changes) | `git reset HEAD~1` | Safe — changes stay in working dir |
| Undo last commit (discard changes) | `git reset --hard HEAD~1` | **Destructive** — loses the commit and changes |
| Revert a pushed commit | `git revert <hash>` | Safe — creates a new commit that undoes the old one |

**Rule:** Use `git revert` for shared history. Use `git reset` only for unpushed commits.

## Stashing

Save work in progress without committing:

```bash
git stash                    # Save current changes
git stash -u                 # Save changes including untracked files
git stash pop                # Restore most recent stash and delete it
git stash apply              # Restore most recent stash but keep it
git stash list               # See all stashes
git stash drop stash@{1}     # Delete a specific stash
```

**When to stash:**
- You need to switch branches but aren't ready to commit
- You want to test something on a clean working directory
- You need to pull changes but have local modifications

## Reading History to Understand Code

```bash
git log --follow -p <file>   # History of a file including renames
git blame <file>             # Who wrote each line and when
git show <commit>            # Full diff of a specific commit
git log -S "function_name"   # Find commits that added/removed a string
git log --since="2 weeks"    # Recent history
```

**Use history to answer:**
- "When was this line changed and why?" → `git blame` + `git show <commit>`
- "What was this code like before this change?" → `git log -p <file>`
- "Who wrote this feature?" → `git log --follow <file>`

## Anti-Patterns

- **Committing unrelated changes together.** Each commit should be one logical change.
- **Vague commit messages.** "fix stuff" is useless in 6 months.
- **Rebasing shared branches.** This rewrites history others depend on.
- **Force pushing to main.** This loses history for everyone.
- **Not checking status before operations.** `git status` takes 1 second and saves you from losing work.
- **Giant merge commits.** If your feature branch diverged significantly, rebase it first to keep history readable.
