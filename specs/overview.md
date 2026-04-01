# AutoDev Overview

AutoDev is a Tauri 2.x desktop app that provides a kanban board view of GitHub issues with automated AI-powered development workflows. When an issue is claimed, AutoDev orchestrates Claude Code sessions through a 3-stage pipeline: Spec → Implement → Review.

## Problem

Existing AI coding tools (Devin, Copilot Coding Agent, Cursor Background Agents, OpenAI Codex) each do one piece well, but none provide the full pipeline of:
- Issue → spec/questions → implement → self-review → PR with test instructions → local dashboard to monitor and test

And critically, none are local-first with a nice desktop UI for managing everything.

## What AutoDev Does Differently

| Problem with existing tools | How AutoDev solves it |
|---|---|
| No spec stage — tools jump straight to implementation | Dedicated spec session reads the full codebase, surfaces questions, ensures Claude has context before writing code |
| No self-review — you're the only reviewer | Separate review session catches issues before you ever see the PR |
| Hard to test locally — "pull the branch" is manual friction | App shows test instructions and runs the test script with one click |
| No pipeline — monolithic agent session | 3 focused sessions: spec, implement, review. Each with a clear job and fresh context |
| Expensive — ACU-based pricing, per-request charges | Uses your existing Claude Code subscription (BYOA) |
| Black box — can't customize the workflow | You own the agent prompts. Edit them in settings per stage |

## Core Flow

1. Create or claim a GitHub issue
2. AutoDev specs it out (reads codebase, identifies files, proposes approach, asks questions)
3. If questions → blocks, waits for your answer
4. If clear → implements in an isolated git worktree
5. Self-reviews the diff, fixes issues, opens a PR
6. You click Test → runs your run script in the worktree
7. You click Merge → squash merges, closes issue, cleans up worktree
8. CI failures and merge conflicts are auto-detected and auto-fixed
