# Settings

## App Settings

Global settings that apply across all repos.

| Setting | Type | Default | Description |
|---|---|---|---|
| Sleep prevention | boolean | true | When any Claude session is running, prevent macOS idle sleep via `caffeinate -i -w <PID>` |
| Notifications | boolean | true | macOS native notifications for Blocked and Ready for Review |
| Poll interval | number (seconds) | 15 | How frequently to poll GitHub for issue updates |

## Agent Prompts

Editable text prompts for each pipeline stage. Each prompt is sent as the system prompt to Claude. Defaults are provided and can be customized.

| Stage | Purpose | Permission Mode |
|---|---|---|
| Spec | Read-only analysis, identify files, propose approach, ask questions | plan |
| Implement | Write code, run tests, follow the spec | bypassPermissions |
| Review | Review diff, fix issues, generate test instructions, create PR | bypassPermissions |
| CI Fix | Read CI logs, diagnose and fix the failure | bypassPermissions |
| Merge Conflict | Resolve merge conflicts against the base branch | bypassPermissions |

Prompts are stored in the `agent_prompts` table. When a prompt is customized, `is_default` is set to 0. The original defaults can be restored.

## Per-Repo Settings

Each connected repo has its own configuration. Stored in the `repos` table.

| Setting | Type | Default | Description |
|---|---|---|---|
| Setup script | text | (empty) | Bash run after worktree creation (e.g., `npm install`) |
| Run script | text | (empty) | Bash run when clicking Test (e.g., `npm run dev`) |
| Base branch | text | `main` | Branch to create worktrees from and merge PRs into |
| Branch prefix | text | `autodev/` | Prefix for worktree branches (e.g., `autodev/issue-42`) |
| Worktree directory | text | `.worktrees/` | Directory within the repo for worktrees |

All per-repo settings are stored locally in SQLite, not in the repo itself. This keeps the repo clean and avoids config files that other team members might not want.

## Sleep Prevention

When enabled and at least one Claude session is running:
- Spawns `caffeinate -i -w <PID>` as a child process
- `-i` prevents idle sleep
- `-w <PID>` ties caffeinate to the app's process — if the app quits, caffeinate dies
- Can be toggled off in settings
