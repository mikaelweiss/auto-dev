# Issue Lifecycle

## Kanban Columns

| Column | GitHub Label | Description |
|---|---|---|
| Backlog | (none) | Issue exists, no work started |
| Claimed | `autodev:claimed` | Stage 1: Spec session running |
| In Progress | `autodev:in-progress` | Stage 2: Implement or Stage 3: Review running |
| Blocked | `autodev:blocked` | Needs human input |
| Ready for Review | `autodev:review` | PR open, waiting for user to test and merge |
| Done | (none) | PR merged, issue closed, worktree cleaned up |

GitHub labels are the source of truth for column position. The app reads labels to determine column placement. When the pipeline changes state, it updates the GitHub label.

## Creating an Issue

### In-app
1. Click [+ New Issue]
2. Form: Title, Body, Assignee (defaults to current user)
3. "Create & Start" → creates on GitHub, auto-claims, starts spec
4. "Create" → creates on GitHub, goes to Backlog

### Via GitHub
1. Issue appears in Backlog via polling
2. When assigned to current user → auto-claims → starts spec

## Stage 1: Spec (Claimed)

- Creates git worktree, runs repo setup script
- Claude session with **read-only permissions** (`--permission-mode plan`)
- Reads issue body + full codebase
- Posts a comment on the GitHub issue with:
  - **Relevant Files**: list of files related to the issue
  - **Proposed Approach**: step-by-step implementation plan
  - **Questions** (if any): numbered list
- If no questions → auto-advances to Stage 2
- If questions → moves to **Blocked**

## Stage 2: Implement (In Progress)

- New Claude session with **full permissions** (`--permission-mode bypassPermissions`)
- System prompt includes the spec comment + issue body
- Writes code, runs tests, creates/updates spec files in repo
- If questions arise mid-implementation → **Blocked**
- When done → auto-advances to Stage 3

## Stage 3: Review (In Progress → Ready for Review)

- Fresh Claude session reviews the diff
- Finds issues → auto-fixes them
- Generates test/reproduction instructions
- Pushes branch to origin
- Opens PR on GitHub
- Moves card to **Ready for Review**

## Blocked

Triggered any time Claude needs human input, during either spec or implementation.

1. Claude posts a comment on the GitHub issue with the question
2. Issue label changes to `autodev:blocked`
3. Card shows a notification badge
4. macOS notification fires
5. User responds either:
   - In the app's detail panel (posted as GitHub comment, session resumes)
   - On the GitHub issue directly (detected via polling, session resumes)
6. Blocked label removed, returns to previous stage

## Ready for Review

- Card shows test instructions and **[▶ Test]** button
- macOS notification fires
- **CI monitoring**: if CI fails, auto-moves back to In Progress, Claude reads CI logs, fixes, pushes, returns to Ready for Review
- **Merge conflict monitoring**: if conflicts detected, same auto-fix loop
- **Loop limit**: 3 attempts for CI fix or merge conflict resolution, then → Blocked
- User clicks **[▶ Test]** → runs repo run script in the worktree
- User clicks **[Merge]** → squash merge via GitHub API

## Done

- PR merged
- GitHub issue closed
- Worktree deleted
- Card moves to Done column

## State Diagram

```
                    ┌───────────┐
  Issue Created ──► │  BACKLOG  │
                    └─────┬─────┘
                          │ assigned to user
                          ▼
                    ┌───────────┐
                    │  CLAIMED  │ ◄── Stage 1: Spec
                    └─────┬─────┘
                          │
              ┌───────────┼───────────┐
              │ questions  │ clear     │
              ▼           ▼           │
        ┌──────────┐  ┌────────────┐  │
        │ BLOCKED  │  │IN PROGRESS │ ◄┘ Stage 2 + 3
        └────┬─────┘  └─────┬──────┘
             │ answered      │ done
             └──► back ◄─────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │READY FOR REVIEW │
                    └────────┬────────┘
                             │
                    ┌────────┼────────┐
                    │ CI fail│ merge  │ conflict
                    │        │        │
                    ▼        ▼        ▼
                  auto-fix loop (max 3)
                             │
                             │ merged
                             ▼
                    ┌──────────┐
                    │   DONE   │
                    └──────────┘
```
