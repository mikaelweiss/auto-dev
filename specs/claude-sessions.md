# Claude Code Sessions

## How Claude Is Invoked

AutoDev spawns the `claude` CLI directly from Rust via `tokio::process::Command`. No Agent SDK, no Node.js. This follows the same BYOA (bring your own auth) model as T3 Code — the user's existing Claude Code subscription or API key is used.

```
claude -p \
  --output-format stream-json \
  --permission-mode {plan|bypassPermissions} \
  --system-prompt "{prompt}" \
  "{user message}"
```

The `--output-format stream-json` flag produces NDJSON (newline-delimited JSON) on stdout, which is read line-by-line and emitted as Tauri events to the frontend.

## Finding the Claude Binary

Checked in order:
1. `/usr/local/bin/claude`
2. `/opt/homebrew/bin/claude`
3. Result of `which claude`

## Session Stages

### Stage 1: Spec

```
Permission mode: plan (read-only)
Purpose: Analyze the issue and codebase, propose approach, surface questions
Output: GitHub comment with relevant files, proposed approach, questions
```

System prompt instructs Claude to:
- Read the issue body carefully
- Explore the full codebase
- Identify files that need modification
- Propose a step-by-step implementation plan
- List clarifying questions
- NOT modify any files

If the output contains questions → issue moves to Blocked.
If output says "No questions — ready to implement" → auto-advance to Stage 2.

### Stage 2: Implement

```
Permission mode: bypassPermissions (full access)
Purpose: Write the code, run tests, make it work
Input: Issue body + spec comment from Stage 1
```

System prompt instructs Claude to:
- Implement the changes from the spec
- Follow existing code style exactly
- Write/update tests
- Run test suite, fix failures
- Ensure clean build
- Make focused commits
- Stop and ask if ambiguity is encountered

On completion → auto-advance to Stage 3.

### Stage 3: Review

```
Permission mode: bypassPermissions (full access)
Purpose: Self-review, fix issues, create PR
Input: Git diff of all changes
```

System prompt instructs Claude to:
- Review the diff for bugs, edge cases, security issues
- Fix anything found
- Run tests again
- Write test/reproduction instructions
- Create a PR with clear title and description

On completion → push branch, create PR, move to Ready for Review.

### CI Fix

```
Permission mode: bypassPermissions
Trigger: Polling detects CI failure on a review PR
Input: CI failure logs
Limit: 3 attempts, then Blocked
```

### Merge Conflict Resolution

```
Permission mode: bypassPermissions
Trigger: Polling detects merge conflicts on a review PR
Input: Base branch name
Limit: 3 attempts, then Blocked
```

## Session State

Each session is tracked in the `sessions` SQLite table with:
- Unique UUID
- Repo ID + issue number
- Stage (spec/implement/review/ci_fix/merge_conflict)
- Worktree path
- Status (running/completed/failed)
- Error message (if failed)
- Timestamps

## Streaming Output

Claude's NDJSON output is parsed line-by-line. Each event is:
1. Logged to the `session_logs` table
2. Emitted as a `session-log` Tauri event to the frontend
3. Displayed in the Card Detail panel's activity log

## Error Handling

- Session crash → status set to `failed`, error stored, `session-error` event emitted
- Card shows error badge, macOS notification fires
- [Retry] button restarts the stage from scratch
- Auto-retry once for transient failures
- CI/merge conflict fix loop: max 3 attempts, then Blocked

## Agent Prompts

All 5 stage prompts are editable in Settings → Agent Prompts. Defaults are seeded on first launch. Changes are saved to the `agent_prompts` table with `is_default = 0`.

The system prompt sent to Claude is: `{agent_prompt}\n\n---\n\n## Context\n\n{issue details, spec, diff, etc.}`
