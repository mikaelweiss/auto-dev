import type { Issue, ChatMessage, AppSettings, RepoSettings } from "./types";

export const currentUser = {
  login: "mikaelweiss",
  avatarUrl: "https://api.dicebear.com/9.x/initials/svg?seed=MW&backgroundColor=6366f1",
};

export const repos = [
  { owner: "acme", name: "web-app", fullName: "acme/web-app" },
  { owner: "acme", name: "api-server", fullName: "acme/api-server" },
  { owner: "acme", name: "mobile-app", fullName: "acme/mobile-app" },
];

function avatar(seed: string): string {
  return `https://api.dicebear.com/9.x/initials/svg?seed=${seed}&backgroundColor=6366f1`;
}

export const fakeIssues: Issue[] = [
  // Backlog
  {
    id: 1,
    number: 42,
    title: "Add dark mode toggle to settings page",
    body: "Users should be able to switch between light and dark mode from the settings page.\n\n## Requirements\n- Toggle switch in settings\n- Persist preference in local storage\n- Apply theme without page reload",
    column: "backlog",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: null,
    stage: null,
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000 * 2),
    prUrl: null,
  },
  {
    id: 2,
    number: 43,
    title: "Implement webhook retry logic",
    body: "Webhooks that fail should be retried up to 3 times with exponential backoff.",
    column: "backlog",
    assignee: null,
    sessionState: null,
    stage: null,
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000 * 5),
    prUrl: null,
  },
  {
    id: 3,
    number: 44,
    title: "Fix pagination on /users endpoint",
    body: "The cursor-based pagination returns duplicate items when items are inserted concurrently.",
    column: "backlog",
    assignee: { login: "sarahc", avatarUrl: avatar("SC") },
    sessionState: null,
    stage: null,
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000 * 8),
    prUrl: null,
  },
  {
    id: 4,
    number: 55,
    title: "Add rate limiting to public API",
    body: "We need rate limiting on public endpoints to prevent abuse.",
    column: "backlog",
    assignee: null,
    sessionState: null,
    stage: null,
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000 * 24),
    prUrl: null,
  },

  // Claimed (spec running)
  {
    id: 5,
    number: 45,
    title: "Migrate auth to OAuth 2.0 PKCE flow",
    body: "Replace the current implicit grant flow with PKCE for better security.",
    column: "claimed",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: "in-progress",
    stage: "spec",
    hasError: false,
    updatedAt: new Date(Date.now() - 60000 * 3),
    prUrl: null,
  },

  // In Progress
  {
    id: 6,
    number: 46,
    title: "Refactor database connection pooling",
    body: "Move from single connection to a connection pool for better performance under load.",
    column: "in-progress",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: "in-progress",
    stage: "implement",
    hasError: false,
    updatedAt: new Date(Date.now() - 60000 * 12),
    prUrl: null,
  },
  {
    id: 7,
    number: 47,
    title: "Add email notification service",
    body: "Implement transactional emails using SendGrid for password resets and account verification.",
    column: "in-progress",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: "canceled",
    stage: "implement",
    hasError: false,
    updatedAt: new Date(Date.now() - 60000 * 45),
    prUrl: null,
  },

  // Blocked
  {
    id: 8,
    number: 48,
    title: "Integrate Stripe billing for teams",
    body: "Add team billing with per-seat pricing through Stripe.",
    column: "blocked",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: null,
    stage: "spec",
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000),
    prUrl: null,
  },

  // Ready for Review
  {
    id: 9,
    number: 49,
    title: "Add CSV export for analytics dashboard",
    body: "Users should be able to export their analytics data as CSV.",
    column: "review",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: null,
    stage: "review",
    hasError: false,
    updatedAt: new Date(Date.now() - 60000 * 30),
    prUrl: "https://github.com/acme/web-app/pull/127",
  },
  {
    id: 10,
    number: 50,
    title: "Fix memory leak in WebSocket handler",
    body: "The WebSocket connection handler isn't cleaning up event listeners on disconnect.",
    column: "review",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: null,
    stage: null,
    hasError: true,
    updatedAt: new Date(Date.now() - 60000 * 5),
    prUrl: "https://github.com/acme/web-app/pull/128",
  },

  // Done
  {
    id: 11,
    number: 51,
    title: "Update dependencies to latest versions",
    body: "Routine dependency update for Q1.",
    column: "done",
    assignee: { login: "mikaelweiss", avatarUrl: avatar("MW") },
    sessionState: null,
    stage: null,
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000 * 48),
    prUrl: "https://github.com/acme/web-app/pull/125",
  },
  {
    id: 12,
    number: 52,
    title: "Add input validation to signup form",
    body: "Client-side and server-side validation for the signup form fields.",
    column: "done",
    assignee: { login: "sarahc", avatarUrl: avatar("SC") },
    sessionState: null,
    stage: null,
    hasError: false,
    updatedAt: new Date(Date.now() - 3600000 * 72),
    prUrl: "https://github.com/acme/web-app/pull/124",
  },
];

export const fakeChatMessages: ChatMessage[] = [
  {
    id: 1,
    role: "assistant",
    content:
      "I'll analyze the codebase to understand the current authentication flow and plan the migration to OAuth 2.0 PKCE.",
    timestamp: new Date(Date.now() - 60000 * 10),
  },
  {
    id: 2,
    role: "tool",
    content: "Read src/auth/handler.ts (245 lines)\nRead src/auth/middleware.ts (89 lines)\nRead src/auth/config.ts (34 lines)",
    toolName: "Read",
    timestamp: new Date(Date.now() - 60000 * 9),
  },
  {
    id: 3,
    role: "assistant",
    content:
      "I've reviewed the current auth implementation. Here's what I found:\n\n**Current state:**\n- Using implicit grant flow in `src/auth/handler.ts`\n- Token stored in localStorage (line 42)\n- No PKCE support\n\n**Proposed approach:**\n1. Add PKCE challenge generation in a new `src/auth/pkce.ts` module\n2. Update `handler.ts` to use authorization code flow\n3. Add token refresh logic\n4. Migrate localStorage token to httpOnly cookie\n\n**Questions:**\n- Should we support both flows during migration, or cut over immediately?\n- Is there a preferred crypto library for the PKCE challenge?",
    timestamp: new Date(Date.now() - 60000 * 8),
  },
  {
    id: 4,
    role: "user",
    content: "Cut over immediately, no need for backwards compat. Use the built-in Web Crypto API.",
    timestamp: new Date(Date.now() - 60000 * 5),
  },
  {
    id: 5,
    role: "assistant",
    content:
      "Got it. I'll implement a clean cutover using Web Crypto API for PKCE challenge generation. Starting implementation now.",
    timestamp: new Date(Date.now() - 60000 * 4),
  },
  {
    id: 6,
    role: "tool",
    content: "Created src/auth/pkce.ts\nModified src/auth/handler.ts (12 hunks)\nModified src/auth/config.ts (3 hunks)\nDeleted src/auth/implicit-flow.ts",
    toolName: "Edit",
    timestamp: new Date(Date.now() - 60000 * 3),
  },
  {
    id: 7,
    role: "assistant",
    content: "Implementation complete. Running tests to verify...",
    timestamp: new Date(Date.now() - 60000 * 2),
  },
  {
    id: 8,
    role: "tool",
    content: "$ npm test -- --grep auth\n\n  Auth PKCE Flow\n    \u2713 generates valid code verifier (2ms)\n    \u2713 generates valid code challenge (3ms)\n    \u2713 completes authorization code exchange (15ms)\n    \u2713 refreshes expired token (8ms)\n    \u2713 handles invalid state parameter (1ms)\n\n  5 passing (29ms)",
    toolName: "Bash",
    timestamp: new Date(Date.now() - 60000),
  },
];

export const defaultAppSettings: AppSettings = {
  sleepPrevention: true,
  notifications: true,
  pollInterval: 15,
};

export const defaultRepoSettings: RepoSettings = {
  setupScript: "bun install",
  runScript: "bun run dev",
  baseBranch: "main",
  branchPrefix: "autodev/",
  worktreeDir: ".worktrees/",
};

export const defaultPrompts: Record<string, string> = {
  spec: `You are a senior software architect. Read the GitHub issue and explore the codebase thoroughly.

Post a GitHub comment with:
1. Relevant files you found
2. Your proposed approach
3. Any clarifying questions

If you have questions, ask them. If the path is clear, say "Ready to implement."`,
  implement: `You are a senior software engineer. You have the spec comment and issue body as context.

Implement the feature or fix:
1. Write clean, idiomatic code
2. Run existing tests and fix any failures
3. Add tests for new functionality
4. If you hit ambiguity, ask for clarification

When done, say "Ready for review."`,
  review: `You are a senior code reviewer. Review the git diff carefully.

Check for:
1. Bugs and edge cases
2. Security issues
3. Performance problems
4. Missing error handling

Fix any issues you find. Run tests. Generate test instructions.
When done, push and create a PR.`,
  "ci-fix": `CI has failed. Read the CI logs and diagnose the failure.

Fix the issue and push. Do not change test expectations unless the test itself is wrong.`,
  "merge-conflict": `There are merge conflicts on this branch. Merge the base branch and resolve all conflicts.

Preserve the intent of both sides. Run tests after resolving.`,
};
