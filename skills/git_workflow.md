# Git Workflow — Branching, Commits, Rebasing

## Daily flow

```bash
git checkout -b feat/my-feature
git add .
git commit -m "feat: add thing"
git rebase main
git push -u origin feat/my-feature
```

## Fix up a commit

```bash
git commit --amend          # fix last commit message
git rebase -i HEAD~3        # squash / reword older commits
```

## Undo

```bash
git reset --soft HEAD~1     # uncommit, keep changes staged
git checkout -- file.txt    # discard unstaged changes
git restore --staged file   # unstage
```
