# Git Worktree Management

## Why Worktrees

Each issue gets its own git worktree so that:
- Multiple issues can be worked on in parallel without file conflicts
- The user's main working tree is never touched
- Each worktree has its own branch, so PRs are clean
- Cleanup is straightforward after merge

## Lifecycle

1. **Created** when an issue enters Claimed (first time Claude works on it)
2. **Used** throughout all stages (spec, implement, review, CI fix, merge conflict)
3. **Deleted** after PR is merged and issue moves to Done

## Naming

- **Branch**: `{branch_prefix}issue-{number}` (e.g., `autodev/issue-42`)
- **Worktree path**: `{repo_path}/{worktree_dir}/issue-{number}/` (e.g., `/Users/me/code/myrepo/.worktrees/issue-42/`)
- Both prefix and directory are configurable per repo in settings

## Creation Flow

```
1. git fetch origin {base_branch}
2. Check if branch already exists (git rev-parse --verify {branch})
3. If worktree directory exists, remove it first (git worktree remove --force)
4. If branch exists:  git worktree add {path} {branch}
   If new:            git worktree add -b {branch} {path} origin/{base_branch}
5. Run setup script in the worktree (if configured)
```

## Setup Script

Per-repo bash script that runs after worktree creation. Examples:
- `npm install`
- `bundle install`
- `pip install -r requirements.txt`
- `cargo build`

Configured in Settings → Repository Settings. Runs with `bash -c "{script}"` in the worktree directory. 5-minute timeout.

## Run Script (Test)

Per-repo bash script that runs when the user clicks [▶ Test]. Examples:
- `npm run dev`
- `cargo run`
- `./scripts/dev.sh`

Output is streamed line-by-line to the frontend via Tauri events. The user can write any bash they want — it runs in the worktree directory.

## Git Operations

All git operations use `tokio::process::Command::new("git")`:

| Operation | Command |
|---|---|
| Create worktree | `git worktree add [-b branch] path [start-point]` |
| Remove worktree | `git worktree remove --force {path}` |
| Delete branch | `git branch -D {branch}` |
| Get diff | `git diff {merge-base}..HEAD` |
| Push | `git push -u origin {branch}` |
| Get branch | `git rev-parse --abbrev-ref HEAD` |
| Fetch | `git fetch origin {branch}` |
| Merge (conflicts) | `git merge origin/{base_branch}` |

## Cleanup

When a PR is merged:
1. `git worktree remove --force {worktree_path}`
2. If remove fails: `rm -rf {worktree_path}` + `git worktree prune`
3. `git branch -D {branch_name}`
