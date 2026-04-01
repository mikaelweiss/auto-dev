# Database Specification

## Storage

- Engine: SQLite via `rusqlite` (bundled)
- Location: `~/.autodev/autodev.db`
- Journal mode: WAL
- Foreign keys: enabled

## Schema

### repos

Tracks which GitHub repos the user has connected.

```sql
CREATE TABLE repos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    github_id INTEGER NOT NULL,
    owner TEXT NOT NULL,
    name TEXT NOT NULL,
    setup_script TEXT NOT NULL DEFAULT '',
    run_script TEXT NOT NULL DEFAULT '',
    base_branch TEXT NOT NULL DEFAULT 'main',
    branch_prefix TEXT NOT NULL DEFAULT 'autodev/',
    worktree_dir TEXT NOT NULL DEFAULT '.worktrees/',
    added_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### sessions

Tracks Claude Code sessions (one per stage per issue attempt).

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    issue_number INTEGER NOT NULL,
    stage TEXT NOT NULL CHECK(stage IN ('spec', 'implement', 'review', 'ci_fix', 'merge_conflict')),
    worktree_path TEXT,
    session_id TEXT,
    status TEXT NOT NULL DEFAULT 'running' CHECK(status IN ('running', 'completed', 'failed')),
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT
);

CREATE INDEX idx_sessions_repo_issue ON sessions(repo_id, issue_number);
```

### auth

Stores GitHub OAuth token.

```sql
CREATE TABLE auth (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL,
    token TEXT NOT NULL,
    username TEXT NOT NULL,
    expires_at TEXT
);
```

### settings

Key-value store for app settings.

```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Default settings:
- `sleep_prevention`: `"true"`
- `notifications_enabled`: `"true"`
- `poll_interval_seconds`: `"15"`

### agent_prompts

Custom prompts for each pipeline stage.

```sql
CREATE TABLE agent_prompts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stage TEXT NOT NULL UNIQUE CHECK(stage IN ('spec', 'implement', 'review', 'ci_fix', 'merge_conflict')),
    prompt_text TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 1
);
```

Seeded on first run with default prompts for all 5 stages.

### session_logs

Streaming log entries from Claude sessions.

```sql
CREATE TABLE session_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    event_type TEXT NOT NULL CHECK(event_type IN ('tool_call', 'message', 'error', 'status_change')),
    content TEXT NOT NULL
);

CREATE INDEX idx_session_logs_session ON session_logs(session_id);
```

## What's NOT in the Database

Issue data, PR data, and board column positions are **not stored** in the database. They come from GitHub via polling. If the database is deleted, the app rebuilds itself from GitHub labels on next poll. Only local config (repo settings, prompts, auth tokens, session history) is persisted.
