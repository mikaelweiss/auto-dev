# UI Specification

## App Layout

```
┌─────────────────────────────────────────────────────┐
│  [Repo Dropdown ▾]              [avatar] [⚙]       │
├──────┬──────┬──────┬──────┬──────────────┬──────────┤
│Back- │Claim-│In    │Block-│Ready for     │Done      │
│log   │ed    │Prog. │ed    │Review        │          │
│      │      │      │      │              │          │
│┌────┐│┌────┐│┌────┐│┌────┐│┌────────────┐│┌────┐   │
││#12 │││#8  │││#5  │││#3  │││#7          │││#1  │   │
││Fix │││Add │││Ref ││││Need│││New login   │││Old ││  │
││bug ││││feat│││act.│││info│││  [▶ Test]  │││feat││  │
│└────┘│└────┘│└────┘│└────┘│└────────────┘│└────┘   │
│      │      │      │      │              │          │
│┌────┐│      │      │      │              │          │
││#14 ││      │      │      │              │          │
││New ││      │      │      │              │          │
│└────┘│      │      │      │              │          │
├──────┴──────┴──────┴──────┴──────────────┴──────────┤
│  [+ New Issue]                              user    │
└─────────────────────────────────────────────────────┘
```

## Window

- Title: "AutoDev"
- Default size: 1400 × 900
- Minimum size: 900 × 600
- Dark theme by default (CSS variables from shadcn-svelte)

## Top Bar

- **Left**: "AutoDev" label + repo selector dropdown
- **Right**: Current user avatar + settings gear button
- Draggable region (`data-tauri-drag-region`)

## Repo Selector

- Dropdown button showing currently selected repo (e.g., "owner/repo")
- Dropdown list of all connected repos
- Click a repo to select it (filters the kanban board)
- "Add Repository" option at bottom opens a dialog (owner + name fields)

## Kanban Board

- 6 columns: Backlog, Claimed, In Progress, Blocked, Ready for Review, Done
- Each column has a header with name and issue count badge
- Columns scroll independently (vertical overflow)
- Drag and drop between columns via `svelte-dnd-action`
- Dropping an issue into a different column triggers the appropriate backend action

## Issue Card

Each card shows:
- Issue title
- `#number`
- Assignee avatar (small circle)
- Time since last update (e.g., "2h ago")
- Stage indicator text (spec / implement / review) if a session is active
- **[▶ Test]** button — only visible in Ready for Review column
- Red error badge — if session crashed
- Orange pulsing dot — if in Blocked column
- Click to open Card Detail panel

## Card Detail Panel

Slide-over from the right side. Shows:
- Issue title (large, editable inline)
- Issue body (with edit toggle)
- Assignee with avatar
- Current session info (stage, status, elapsed time)
- **Activity log**: scrollable list of session log entries (auto-scrolls to bottom)
  - Tool calls: monospace, muted background
  - Messages: normal text
  - Errors: red
- If **Blocked**: text input + send button to respond
- If **Ready for Review**: [▶ Test] button + [Merge] button
- Link to GitHub issue (opens in browser)
- Close button (X) in top-right

## New Issue Dialog

Modal dialog with:
- Title input
- Body textarea
- Assignee (defaults to current user)
- "Create" button (goes to Backlog)
- "Create & Start" button (creates + auto-claims)

## Settings Dialog

Modal with sections:

### App Settings
- Sleep prevention toggle (default: on)
- Notifications toggle
- Poll interval (seconds)

### Agent Prompts
- Editable textarea for each stage: Spec, Implement, Review, CI Fix, Merge Conflict
- Each shows the current prompt (default or custom)

### Repository Settings (per selected repo)
- Setup script (textarea) — runs on worktree creation
- Run script (textarea) — runs when you click Test
- Base branch (input, default: `main`)
- Branch prefix (input, default: `autodev/`)
- Worktree directory (input, default: `.worktrees/`)

## Login Screen

Shown when not authenticated. Contains:
1. "AutoDev" heading
2. "Connect your GitHub account to get started"
3. "Sign in with GitHub" button
4. When auth starts: shows device code, "Open GitHub" link, "Waiting for authorization..."
5. Loading spinner while initializing

## Style

- Dark theme with shadcn-svelte CSS variables
- Clean, minimal feel (like Linear or Notion)
- Rounded corners (`rounded-lg`, `rounded-md`)
- Subtle card shadows
- Consistent spacing (p-3, p-4, gap-3, gap-4)
- `-webkit-user-select: none` for desktop feel (except inputs)
- Custom scrollbar styling (thin, subtle)

## Notifications

- macOS native notifications via `tauri-plugin-notification`
- Fire when:
  - An issue moves to **Blocked** (needs your input)
  - An issue moves to **Ready for Review** (ready to test)
