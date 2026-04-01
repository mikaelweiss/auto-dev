# Error Handling

## Principles

1. **No silent failures** — the user always sees what went wrong
2. **Auto-retry once** for transient failures (network timeout, rate limit)
3. **Loop limits** for automated fix attempts (CI, merge conflicts)
4. **Graceful degradation** — one failed session doesn't break the app

## Claude Session Failures

When a Claude CLI session crashes or exits with an error:

1. Session status → `failed`, error message stored in DB
2. `session-error` Tauri event emitted to frontend
3. Card shows a **red error badge** (distinct from blocked badge)
4. macOS notification: "Issue #12 failed: [truncated error]"
5. Card detail panel shows the full error log
6. **[Retry]** button on the card restarts the current stage from scratch

### Transient vs Permanent Failures

- Network timeout, rate limit → auto-retry once, then fail
- Claude CLI not found → fail immediately with clear message
- Permission denied → fail immediately
- Session killed by user → mark as failed with "Stopped by user"

## CI Fix Loop

When CI fails on a PR in Ready for Review:

1. Auto-move back to In Progress
2. Start a CI fix session (reads CI logs, fixes the issue, pushes)
3. Move back to Ready for Review
4. If CI fails again → repeat (up to 3 attempts)
5. After 3 failures → move to Blocked with message "CI has failed 3 times. Manual intervention required."

## Merge Conflict Loop

Same pattern as CI fix:

1. Auto-move back to In Progress
2. Start merge conflict resolution session
3. Move back to Ready for Review
4. Repeat up to 3 times
5. After 3 → Blocked

## GitHub API Errors

- 401 Unauthorized → clear auth, show login screen
- 403 Rate Limited → wait and retry (respect `Retry-After` header)
- 404 Not Found → log and skip (label might not exist, etc.)
- 5xx → retry once, then log error
- Network failure → retry once, then log error

## Database Errors

- DB file missing → recreated on app launch (migrations run on init)
- Corrupt DB → error shown to user, suggest deleting `~/.autodev/autodev.db`
- Lock contention → `std::sync::Mutex` ensures sequential access

## Process Spawning Errors

- `node` not found → not applicable anymore (no sidecar)
- `claude` not found → clear error: "Claude CLI not found. Install it from https://claude.ai/download"
- `git` not found → clear error: "Git not found in PATH"
- Setup/run script fails → error shown in activity log, non-blocking
