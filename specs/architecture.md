# Architecture

## Two-Layer Design

AutoDev uses a simple two-layer architecture with no sidecar or middleware:

```
┌──────────────────────────────┐
│  Tauri App                   │
│                              │
│  Svelte Frontend             │
│  ├─ Kanban board UI          │
│  ├─ State management         │
│  ├─ Calls invoke() for       │
│  │   all backend operations  │
│  └─ Listens to Tauri events  │
│     for streaming updates    │
│                              │
│  Rust Backend                │
│  ├─ GitHub API (reqwest)     │
│  ├─ SQLite (rusqlite)        │
│  ├─ Claude CLI spawning      │
│  │   (tokio::process)        │
│  ├─ Git worktree ops         │
│  ├─ Polling (tokio task)     │
│  └─ caffeinate               │
│                              │
└──────────────────────────────┘
```

## Why No Sidecar

Early designs included a Node.js sidecar to use the Claude Agent SDK. This was removed because:
- The Agent SDK just wraps the `claude` CLI — we can spawn it directly from Rust
- GitHub API calls can be made with `reqwest` from Rust
- SQLite is simpler with `rusqlite` than `better-sqlite3` (no native module rebuild issues)
- Eliminates an entire layer: no WebSocket protocol, no Node.js runtime dependency, no esbuild step

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2.x (Rust backend) |
| Frontend | Svelte 5 (runes) + TypeScript + shadcn-svelte + Tailwind CSS v3 |
| HTTP client | reqwest (Rust) |
| Database | rusqlite with bundled SQLite |
| Git | `tokio::process::Command` spawning `git` |
| Claude | `tokio::process::Command` spawning `claude` CLI |
| Drag & drop | svelte-dnd-action |
| Icons | lucide-svelte |
| Auth (GitHub) | OAuth device flow |
| Auth (Claude) | BYOA — reuses existing `claude` CLI auth |

## Communication Pattern

**Frontend → Backend**: `invoke('command_name', { args })` via `@tauri-apps/api/core`

**Backend → Frontend**: `app_handle.emit("event-name", payload)` via Tauri events, listened to with `listen()` from `@tauri-apps/api/event`

This replaces the old WebSocket protocol entirely. The frontend is always "connected" since it communicates directly with the Rust process.

## Rust Module Structure

```
src-tauri/src/
├── main.rs          # Entry point, calls lib::run()
├── lib.rs           # Tauri setup, AppState, command registration
├── types.rs         # Serde structs matching frontend TypeScript types
├── db.rs            # SQLite: open, migrate, CRUD for all tables
├── github.rs        # GitHub API: auth, issues, labels, PRs, comments
├── sessions.rs      # Claude CLI session management, settings, prompts
├── worktrees.rs     # Git worktree operations
└── polling.rs       # Background polling with tokio tasks
```

## Shared State

```rust
pub struct AppState {
    pub db: std::sync::Mutex<rusqlite::Connection>,
    pub http_client: reqwest::Client,
}
```

Managed via `app.manage(state)` in Tauri's `setup()` hook. Accessed in commands via `state: tauri::State<'_, Arc<AppState>>`.

The `rusqlite::Connection` is not `Send`, so it uses `std::sync::Mutex` (not tokio's). The lock is always acquired and released within synchronous scope, never held across an `.await`.
