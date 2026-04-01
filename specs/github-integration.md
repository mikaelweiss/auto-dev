# GitHub Integration

## Authentication

### OAuth Device Flow

AutoDev uses GitHub's OAuth device flow, which is the standard for desktop apps:

1. App requests a device code from `POST https://github.com/login/device/code`
   - Sends `client_id` and `scope: repo`
2. User sees a code in the app (e.g., `ABCD-1234`)
3. User clicks "Open GitHub" → opens `https://github.com/login/device` in browser
4. User enters the code and authorizes
5. App polls `POST https://github.com/login/oauth/access_token` until token is returned
6. Token is stored in SQLite (`auth` table)
7. All subsequent API calls use `Authorization: Bearer {token}`

Requires registering a GitHub OAuth App (free). The client ID is compiled into the app.

### Token Storage

- Stored in `~/.autodev/autodev.db` in the `auth` table
- Provider: `github`
- On logout: row deleted, in-memory client cleared

## Sync Method

### Polling with ETags

- Poll every 15 seconds (configurable)
- Use `If-None-Match` header with ETag for conditional requests
- `304 Not Modified` responses do **not count** against rate limits
- Single REST endpoint: `GET /repos/{owner}/{name}/issues?state=all&per_page=100&sort=updated`
- Polls all tracked repos in parallel

### Why Not Webhooks

Webhooks require a publicly reachable endpoint. A desktop app behind NAT can't receive them without a tunnel (ngrok, smee.io), and those are fragile for always-on use. Polling with ETags is simple, reliable, and has negligible rate limit impact.

### Why Not GraphQL Subscriptions

GitHub does not support GraphQL subscriptions (WebSocket-based real-time updates). Only queries and mutations are available.

## Labels as Source of Truth

GitHub labels determine board column position:

| Label | Column |
|---|---|
| (none) | Backlog |
| `autodev:claimed` | Claimed |
| `autodev:in-progress` | In Progress |
| `autodev:blocked` | Blocked |
| `autodev:review` | Ready for Review |
| (issue closed) | Done |

When a repo is first added, AutoDev ensures these labels exist (creates them if missing):
- `autodev:claimed` — green (#0E8A16)
- `autodev:in-progress` — blue (#1D76DB)
- `autodev:blocked` — red (#D93F0B)
- `autodev:review` — yellow (#FBCA04)

## Team Use

GitHub IS the shared state. Each team member runs their own Tauri app. Since everyone polls the same issues/labels, everyone sees the same board. No shared backend needed.

- Filter by assignee comes naturally (GitHub issues have assignees)
- The app only auto-starts Claude for issues assigned to the current user
- Other team members' progress is visible via labels

## Polling Detections

Beyond syncing issues, polling also detects:

1. **New comments on blocked issues**: If a blocked issue gets a new comment, the session is automatically resumed with that comment as the user's response.

2. **CI failures on review PRs**: If a PR tied to a `autodev:review` issue has failing checks, a CI fix session is triggered.

3. **Merge conflicts**: If a PR has `mergeable: false` with `mergeable_state: "dirty"`, a merge conflict resolution session is triggered.

## API Endpoints Used

| Operation | Method | Endpoint |
|---|---|---|
| Device code | POST | `https://github.com/login/device/code` |
| Token exchange | POST | `https://github.com/login/oauth/access_token` |
| Current user | GET | `/user` |
| Repo info | GET | `/repos/{owner}/{name}` |
| List issues | GET | `/repos/{owner}/{name}/issues` |
| Create issue | POST | `/repos/{owner}/{name}/issues` |
| Get issue | GET | `/repos/{owner}/{name}/issues/{number}` |
| Close issue | PATCH | `/repos/{owner}/{name}/issues/{number}` |
| Add label | POST | `/repos/{owner}/{name}/issues/{number}/labels` |
| Remove label | DELETE | `/repos/{owner}/{name}/issues/{number}/labels/{label}` |
| List comments | GET | `/repos/{owner}/{name}/issues/{number}/comments` |
| Create comment | POST | `/repos/{owner}/{name}/issues/{number}/comments` |
| Create/get label | POST/GET | `/repos/{owner}/{name}/labels` |
| Create PR | POST | `/repos/{owner}/{name}/pulls` |
| Merge PR | PUT | `/repos/{owner}/{name}/pulls/{number}/merge` |
| PR status | GET | `/repos/{owner}/{name}/pulls/{number}` |
| Combined status | GET | `/repos/{owner}/{name}/commits/{ref}/status` |
| Check runs | GET | `/repos/{owner}/{name}/commits/{ref}/check-runs` |

Base URL: `https://api.github.com`
Headers: `Authorization: Bearer {token}`, `Accept: application/vnd.github+json`, `User-Agent: AutoDev`
