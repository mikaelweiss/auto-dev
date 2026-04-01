import type { Issue, Column, AppSettings, RepoSettings } from "./types";
import {
  fakeIssues,
  repos,
  currentUser,
  defaultAppSettings,
  defaultRepoSettings,
  defaultPrompts,
} from "./fake-data";

// App state using Svelte 5 runes
let issues = $state<Issue[]>(fakeIssues);
let selectedRepo = $state(repos[0]);
let selectedIssue = $state<Issue | null>(null);
let showNewIssueDialog = $state(false);
let showSettingsDialog = $state(false);
let appSettings = $state<AppSettings>({ ...defaultAppSettings });
let repoSettings = $state<RepoSettings>({ ...defaultRepoSettings });
let prompts = $state<Record<string, string>>({ ...defaultPrompts });

export function getIssues() {
  return issues;
}

export function getIssuesByColumn(column: Column): Issue[] {
  return issues.filter((i) => i.column === column);
}

export function getSelectedRepo() {
  return selectedRepo;
}

export function setSelectedRepo(repo: (typeof repos)[0]) {
  selectedRepo = repo;
}

export function getRepos() {
  return repos;
}

export function getCurrentUser() {
  return currentUser;
}

export function getSelectedIssue() {
  return selectedIssue;
}

export function setSelectedIssue(issue: Issue | null) {
  selectedIssue = issue;
}

export function getShowNewIssueDialog() {
  return showNewIssueDialog;
}

export function setShowNewIssueDialog(show: boolean) {
  showNewIssueDialog = show;
}

export function getShowSettingsDialog() {
  return showSettingsDialog;
}

export function setShowSettingsDialog(show: boolean) {
  showSettingsDialog = show;
}

export function getAppSettings() {
  return appSettings;
}

export function setAppSettings(s: AppSettings) {
  appSettings = s;
}

export function getRepoSettings() {
  return repoSettings;
}

export function setRepoSettings(s: RepoSettings) {
  repoSettings = s;
}

export function getPrompts() {
  return prompts;
}

export function setPrompt(key: string, value: string) {
  prompts[key] = value;
}

export function moveIssue(issueId: number, toColumn: Column) {
  const issue = issues.find((i) => i.id === issueId);
  if (!issue) return;
  issue.column = toColumn;
  issue.updatedAt = new Date();
}

export function stopSession(issueId: number) {
  const issue = issues.find((i) => i.id === issueId);
  if (!issue) return;
  issue.sessionState = "canceled";
}

export function resumeSession(issueId: number) {
  const issue = issues.find((i) => i.id === issueId);
  if (!issue) return;
  issue.sessionState = "in-progress";
  issue.updatedAt = new Date();
}

export function addIssue(title: string, body: string, startImmediately: boolean) {
  const maxNum = Math.max(...issues.map((i) => i.number));
  const newIssue: Issue = {
    id: Date.now(),
    number: maxNum + 1,
    title,
    body,
    column: startImmediately ? "claimed" : "backlog",
    assignee: { ...currentUser },
    sessionState: startImmediately ? "initializing" : null,
    stage: startImmediately ? "spec" : null,
    hasError: false,
    updatedAt: new Date(),
    prUrl: null,
  };
  issues.push(newIssue);
  if (startImmediately) {
    // Simulate session starting after a moment
    setTimeout(() => {
      newIssue.sessionState = "in-progress";
    }, 2000);
  }
}
