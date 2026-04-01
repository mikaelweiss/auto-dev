# AutoDev Spec

## What It Is

AutoDev is a desktop app that turns GitHub issues into finished PRs through a 3-stage AI pipeline: **Spec → Implement → Review**. It presents issues as a kanban board and orchestrates Claude Code sessions to do the work.

## Why It Exists

Existing AI coding tools (Devin, Copilot Coding Agent, Cursor Background Agents, Codex) each do one piece but none provide the full pipeline:

- **No spec stage** — they jump straight to implementation without ensuring Claude has context
- **No self-review** — you're the only reviewer
- **No local testing** — "pull the branch" is manual friction
- **No pipeline** — monolithic agent sessions with no separation of concerns
- **Expensive** — ACU-based pricing or per-request charges
- **Black box** — can't customize the workflow

AutoDev is local-first, uses your existing Claude Code subscription (BYOA), and gives you a desktop UI to manage everything.

## Tech Stack

- **Desktop shell**: Tauri 2.x (Rust backend, no sidecar)
- **Frontend**: Svelte 5 (runes) + TypeScript + shadcn-svelte + Tailwind CSS v3
- **Database**: SQLite (bundled, stored at `~/.autodev/autodev.db`)
- **AI**: Spawns the `claude` CLI directly from Rust (no Agent SDK, no Node.js)
- **Drag & drop**: svelte-dnd-action
- **Icons**: lucide-svelte

## Core Flow

1. Create or claim a GitHub issue
2. **Spec**: Claude reads the codebase in read-only mode, proposes an approach, asks questions
3. If questions → issue blocks, waits for human answer (in-app or on GitHub)
4. If clear → **Implement**: Claude writes code in an isolated git worktree with full permissions
5. **Review**: A fresh Claude session reviews the diff, fixes issues, opens a PR
6. User clicks **Test** → runs a configurable script in the worktree
7. User clicks **Merge** → squash merge, close issue, clean up worktree
8. CI failures and merge conflicts are auto-detected and auto-fixed (up to 3 attempts each, then blocked)

## Kanban Board

Six columns, driven entirely by GitHub labels:

| Column | Label | What's Happening |
|---|---|---|
| Backlog | (none) | No work started |
| Claimed | `autodev:claimed` | Spec session running |
| In Progress | `autodev:in-progress` | Implement or Review running |
| Blocked | `autodev:blocked` | Needs human input |
| Ready for Review | `autodev:review` | PR open, waiting for user |
| Done | (issue closed) | PR merged, cleaned up |

Labels are the source of truth. When a repo is first added, AutoDev ensures the `autodev:*` labels exist on GitHub.

## GitHub Integration

- **Auth**: GitHub OAuth device flow (user enters a code in the browser)
- **Sync**: Polls GitHub REST API with ETags every 15 seconds (configurable). 304s don't count against rate limits. No webhooks needed.
- **Team use**: GitHub is the shared state. Each team member runs their own app. Everyone sees the same board because everyone polls the same labels. The app only auto-starts Claude for issues assigned to the current user.

### Polling Detections

Beyond syncing issues, polling also detects:
- **New comments on blocked issues** → automatically resumes the session with the comment
- **CI failures on review PRs** → triggers a CI fix session
- **Merge conflicts on review PRs** → triggers a merge conflict resolution session

## Pipeline Stages

### Stage 1: Spec
- Read-only (Claude cannot modify files)
- Reads the issue and explores the full codebase
- Posts a GitHub comment with: relevant files, proposed approach, questions
- If questions → Blocked. If clear → auto-advances to Implement.

### Stage 2: Implement
- Full permissions
- Receives the spec comment + issue body as context
- Writes code, runs tests, fixes failures
- If ambiguity → Blocked. When done → auto-advances to Review.

### Stage 3: Review
- Full permissions, fresh session
- Reviews the git diff for bugs, edge cases, security issues
- Fixes anything found, runs tests again
- Generates test/reproduction instructions
- Pushes branch, creates PR, moves card to Ready for Review

### CI Fix (automated)
- Triggered when polling detects CI failure on a review PR
- Reads CI logs, diagnoses, fixes, pushes
- Max 3 attempts, then Blocked

### Merge Conflict Resolution (automated)
- Triggered when polling detects conflicts on a review PR
- Merges base branch, resolves conflicts
- Max 3 attempts, then Blocked

## Agent Prompts

All 5 stage prompts (Spec, Implement, Review, CI Fix, Merge Conflict) are user-editable with restorable defaults. They are sent as the system prompt to Claude.

## Git Worktrees

Each issue gets its own git worktree so multiple issues can run in parallel without touching the user's main working tree. Worktrees are created when an issue is first claimed and deleted after the PR is merged.

- **Branch naming**: `{configurable prefix}issue-{number}` (default: `autodev/issue-42`)
- **Worktree path**: `{repo path}/{configurable dir}/issue-{number}/` (default: `.worktrees/issue-42/`)

## Settings

### App Settings
| Setting | Default | Description |
|---|---|---|
| Sleep prevention | on | Prevents macOS idle sleep while Claude sessions are running (via `caffeinate`) |
| Notifications | on | macOS native notifications for Blocked and Ready for Review |
| Poll interval | 15s | How often to poll GitHub |

### Per-Repo Settings
| Setting | Default | Description |
|---|---|---|
| Setup script | (empty) | Bash run after worktree creation (e.g., `npm install`) |
| Run script | (empty) | Bash run when clicking Test (e.g., `npm run dev`) |
| Base branch | `main` | Branch to create worktrees from and merge into |
| Branch prefix | `autodev/` | Prefix for worktree branches |
| Worktree directory | `.worktrees/` | Directory within the repo for worktrees |

All settings are stored locally in SQLite, not in the repo.

## UI

### Layout
- **Top bar**: "AutoDev" label, repo selector dropdown, user avatar, settings gear
- **Main area**: 6-column kanban board with independently scrolling columns
- **Bottom**: New Issue button

### Issue Cards
- Title, `#number`, assignee avatar, time since last update
- Stage indicator if a session is active
- **[Test]** button in Ready for Review column
- Red error badge if session crashed
- Orange pulsing dot if Blocked
- Click to open detail panel

### Card Detail Panel
- Slide-over from right
- Issue title and body (editable)
- Current session info (stage, status, elapsed time)
- Activity log: scrollable stream of session output (tool calls, messages, errors)
- If Blocked: text input to respond
- If Ready for Review: Test and Merge buttons
- Link to GitHub issue

### Other Screens
- **Login screen**: GitHub OAuth device flow (shows code, "Open GitHub" link, waiting spinner)
- **New Issue dialog**: title, body, assignee, "Create" or "Create & Start"
- **Settings dialog**: app settings, agent prompts (per stage), repo settings (per repo)

### Style
- Dark theme (shadcn-svelte CSS variables)
- Clean, minimal (Linear/Notion feel)
- Window: 1400x900 default, 900x600 minimum

## Error Handling

- **No silent failures** — the user always sees what went wrong
- **Auto-retry once** for transient failures (network timeout, rate limit)
- **Loop limits** for automated fixes (3 attempts for CI and merge conflicts)
- Session crashes show a red badge, fire a notification, and offer a Retry button
- GitHub 401 → show login screen. 403 rate limit → wait and retry. 5xx/network → retry once.
- If `claude` CLI or `git` not found → clear error message

## Data Model

The database stores only local config: repo settings, auth token, agent prompts, session history, and session logs. **Issue data, PR data, and board positions are never stored** — they come from GitHub via polling. If the database is deleted, the app rebuilds from GitHub labels on next poll.
