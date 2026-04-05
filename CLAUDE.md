# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is AutoDev

AutoDev is a Tauri 2 desktop app that turns GitHub issues into finished PRs through a 3-stage AI pipeline (Spec -> Implement -> Review). It presents issues as a kanban board and orchestrates Claude Code CLI sessions to do the work. See `specs/spec.md` for the full product spec.

## Commands

```bash
# First-time setup (install deps, generate types, check Rust compiles)
./setup.sh

# Run the full Tauri desktop app (Vite + Rust)
./dev.sh

# Run only the Vite frontend (no Rust/Tauri)
bun run vite:dev

# Type-check the Svelte/TS frontend
bun run check

# Build Rust backend only
cargo check --manifest-path src-tauri/Cargo.toml

# Build the full app for release
bun run build && cd src-tauri && cargo build --release
```

There are no tests yet.

## Architecture

**Two-process model**: Tauri Rust backend (`src-tauri/`) + Svelte 5 frontend (`src/`). They communicate via Tauri's `invoke` (frontend->backend commands) and `emit` (backend->frontend events).

### Rust Backend (`src-tauri/src/`)

- `lib.rs` — App entry point. Registers `AppState` (SQLite connection + HTTP client), starts polling, registers all Tauri commands.
- `db.rs` — All SQLite operations. Database lives at `~/.autodev/autodev.db`. Tables: `repos`, `sessions`, `auth`, `settings`, `agent_prompts`, `session_logs`, `issue_state`.
- `issue_state.rs` — Tauri commands for reading/writing per-issue column state (`get_issue_states`, `set_issue_column`).
- `github.rs` — GitHub REST API calls (auth via `gh` CLI token, issues, PRs, merge).
- `sessions.rs` — Core session lifecycle. Spawns `claude` CLI as a subprocess with `--output-format stream-json`, streams stdout into session logs and Tauri events. Three entry points: `session_start` (spec), `session_start_implement`, `session_start_review`. Permission modes: `plan` for spec, `bypassPermissions` for implement/review.
- `worktrees.rs` — Git worktree creation/removal, setup script execution, diff generation, branch pushing.
- `polling.rs` — Background task that polls GitHub issues on an interval and emits `issues-updated` events.
- `types.rs` — All shared Rust types (Issue, Session, RepoConfig, etc).

### Svelte Frontend (`src/`)

- `src/lib/types/index.ts` — TypeScript types mirroring the Rust types. Also defines `COLUMN_CONFIG`, `COLUMN_ORDER`, and `getColumnForSession()`.
- `src/lib/stores/backend.ts` — Typed wrapper around all `invoke()` calls. Single import point for all backend communication.
- `src/lib/stores/` — Svelte stores for reactive state: `issues.ts` (with derived `issuesByColumn`), `sessions.ts` (with derived `sessionByIssue`), `repos.ts`, `auth.ts`, `settings.ts`, `ui.ts`.
- `src/lib/components/` — UI components: `KanbanBoard`, `KanbanColumn`, `IssueCard`, `CardDetail` (modal), `RepoSelector`, `SettingsDialog`, `NewIssueDialog`, `AddRepoDialog`, `RemoveRepoDialog`, `AgentLog`.
- `src/routes/+page.svelte` — Root page. Handles auth gate, top bar, kanban board, bottom bar, and overlay dialogs.

### Key Data Flow

1. Board column position is persisted locally in the `issue_state` SQLite table. Issues default to `backlog` when no state exists.
2. Local session state overrides persisted column state while a session is active (`sessionByIssue` derived store).
3. Polling emits `issues-updated` events; session lifecycle emits `session-status` and `session-log` events. Frontend stores react to both.
4. Issues themselves are never stored locally — they're fetched from GitHub on every poll. Sessions, auth, settings, prompts, and issue column state are persisted in SQLite.

## Tech Stack

- **Frontend**: Svelte 5 (runes), SvelteKit with static adapter, TypeScript, Tailwind CSS v3, shadcn-svelte (bits-ui), lucide-svelte icons, svelte-dnd-action
- **Backend**: Rust (Tauri 2), rusqlite (bundled SQLite), reqwest, tokio, chrono, serde
- **Package manager**: Bun (not npm/yarn)
- **AI integration**: Spawns `claude` CLI directly as a subprocess (not the Agent SDK)

## Conventions

- Kanban columns are identified by `ColumnId` type: `backlog | planning | in_progress | blocked | review | done`
- Session stages: `spec | implement | review | ci_fix | merge_conflict`
- Session statuses: `initializing | setup | running | completed | failed`
- Branch naming: `{branch_prefix}issue-{number}` (default: `autodev/issue-42`)
- Worktree path: `{repo_path}/{worktree_dir}/issue-{number}/`
- All Tauri commands are registered in `lib.rs` and wrapped in `src/lib/stores/backend.ts`
- Frontend uses Svelte 5 runes (`$state`, `$derived`, `$effect`) in components, but stores still use `writable`/`derived` from `svelte/store`

## Other
This is a greenfield project that was made a couple days ago. Don't do anything to be backwards compatible since there are no users yet.
