export type Column =
  | "backlog"
  | "claimed"
  | "in-progress"
  | "blocked"
  | "review"
  | "done";

export type SessionState =
  | "initializing"
  | "in-progress"
  | "canceled"
  | null;

export type Stage = "spec" | "implement" | "review" | "ci-fix" | "merge-conflict";

export interface Issue {
  id: number;
  number: number;
  title: string;
  body: string;
  column: Column;
  assignee: {
    login: string;
    avatarUrl: string;
  } | null;
  sessionState: SessionState;
  stage: Stage | null;
  hasError: boolean;
  updatedAt: Date;
  prUrl: string | null;
}

export interface ChatMessage {
  id: number;
  role: "assistant" | "user" | "tool";
  content: string;
  toolName?: string;
  timestamp: Date;
}

export interface RepoSettings {
  setupScript: string;
  runScript: string;
  baseBranch: string;
  branchPrefix: string;
  worktreeDir: string;
}

export interface AppSettings {
  sleepPrevention: boolean;
  notifications: boolean;
  pollInterval: number;
}
