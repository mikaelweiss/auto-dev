import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
	Issue,
	Session,
	SessionLogEntry,
	GitHubUser,
	GitHubRepo,
	RepoConfig,
	RepoRemovalInfo,
	AppSettings,
	AgentPrompt,
	SessionStage,
	ModelInfo,
	ColumnId
} from '$lib/types';
import { issues, issueStates } from './issues';
import { sessions, sessionLogs } from './sessions';

// Initialize: set up event listeners for Rust -> frontend events
export async function initBackend() {
	// Load persisted sessions from DB so session state survives app restarts
	try {
		const persisted: Session[] = await invoke('session_list');
		sessions.set(persisted);
	} catch (_) {
		// Failed to load sessions
	}

	await listen<{ issues: Issue[]; repo_owner: string; repo_name: string }>(
		'issues-updated',
		(event) => {
			issues.update((current) => {
				const updated = [...current];
				for (const incoming of event.payload.issues) {
					const idx = updated.findIndex(
						(i) =>
							i.number === incoming.number &&
							i.repo_owner === incoming.repo_owner &&
							i.repo_name === incoming.repo_name
					);
					if (idx >= 0) {
						updated[idx] = incoming;
					} else {
						updated.push(incoming);
					}
				}
				return updated;
			});
		}
	);

	await listen<Session>('session-status', (event) => {
		sessions.update((current) => {
			const idx = current.findIndex((s) => s.id === event.payload.id);
			if (idx >= 0) {
				current[idx] = event.payload;
				return [...current];
			}
			return [...current, event.payload];
		});
	});

	await listen<{ session_id: string; entry: SessionLogEntry }>('session-log', (event) => {
		sessionLogs.update((current) => {
			const logs = current.get(event.payload.session_id) ?? [];
			logs.push(event.payload.entry);
			current.set(event.payload.session_id, logs);
			return new Map(current);
		});
	});

	await listen<{ session_id: string; question: string }>('session-blocked', (event) => {
		sessionLogs.update((current) => {
			const logs = current.get(event.payload.session_id) ?? [];
			logs.push({
				id: crypto.randomUUID(),
				session_id: event.payload.session_id,
				timestamp: new Date().toISOString(),
				event_type: 'message',
				content: event.payload.question
			});
			current.set(event.payload.session_id, logs);
			return new Map(current);
		});
	});

	await listen<{ session_id: string; error: string }>('session-error', (event) => {
		sessionLogs.update((current) => {
			const logs = current.get(event.payload.session_id) ?? [];
			logs.push({
				id: crypto.randomUUID(),
				session_id: event.payload.session_id,
				timestamp: new Date().toISOString(),
				event_type: 'error',
				content: event.payload.error
			});
			current.set(event.payload.session_id, logs);
			return new Map(current);
		});
	});

	await listen<{ session_id: string; line: string }>('test-output', (event) => {
		sessionLogs.update((current) => {
			const logs = current.get(event.payload.session_id) ?? [];
			logs.push({
				id: crypto.randomUUID(),
				session_id: event.payload.session_id,
				timestamp: new Date().toISOString(),
				event_type: 'test_output',
				content: event.payload.line
			});
			current.set(event.payload.session_id, logs);
			return new Map(current);
		});
	});
}

export async function loadIssueStates(repoId: number) {
	try {
		const states = await getIssueStates(repoId);
		issueStates.update((current) => {
			for (const [issueNumber, columnId] of states) {
				current.set(`${repoId}:${issueNumber}`, columnId as ColumnId);
			}
			return new Map(current);
		});
	} catch (_) {
		// Failed to load issue states
	}
}

// Auth
export async function authFromCli(): Promise<GitHubUser> {
	return invoke('github_auth_from_cli');
}

export async function getAuthStatus(): Promise<{
	authenticated: boolean;
	user: GitHubUser | null;
}> {
	return invoke('github_get_auth_status');
}

// Repos
export async function listUserRepos(): Promise<GitHubRepo[]> {
	return invoke('github_list_user_repos');
}

export async function addRepo(owner: string, name: string): Promise<RepoConfig> {
	return invoke('github_add_repo', { owner, name });
}

export async function addLocalRepo(path: string): Promise<RepoConfig> {
	return invoke('github_add_local_repo', { path });
}

export async function getRepoRemovalInfo(repoId: number): Promise<RepoRemovalInfo> {
	return invoke('github_get_repo_removal_info', { repoId });
}

export async function removeRepo(repoId: number): Promise<void> {
	return invoke('github_remove_repo', { repoId });
}

export async function getRepos(): Promise<RepoConfig[]> {
	return invoke('github_get_repos');
}

// Collaborators
export async function listCollaborators(owner: string, name: string): Promise<GitHubUser[]> {
	return invoke('github_list_collaborators', { owner, name });
}

// Issues
export async function fetchIssues(owner: string, name: string): Promise<Issue[]> {
	return invoke('github_fetch_issues', { owner, name });
}

export async function createIssue(
	owner: string,
	name: string,
	title: string,
	body: string,
	assignee: string | null
): Promise<Issue> {
	return invoke('github_create_issue', { owner, name, title, body, assignee });
}

// Sessions
export async function startSession(
	repoId: number,
	issueNumber: number,
	message?: string,
	modelOverride?: string,
	effortOverride?: string
): Promise<void> {
	return invoke('session_start', {
		repoId,
		issueNumber,
		message: message ?? null,
		modelOverride: modelOverride ?? null,
		effortOverride: effortOverride ?? null
	});
}

export async function startImplementSession(repoId: number, issueNumber: number): Promise<void> {
	return invoke('session_start_implement', { repoId, issueNumber });
}

export async function startReviewSession(repoId: number, issueNumber: number): Promise<void> {
	return invoke('session_start_review', { repoId, issueNumber });
}

export async function respondToSession(
	sessionId: string,
	message: string,
	modelOverride?: string,
	effortOverride?: string
): Promise<void> {
	return invoke('session_respond', {
		sessionId,
		message,
		modelOverride: modelOverride ?? null,
		effortOverride: effortOverride ?? null
	});
}

export async function retrySession(sessionId: string): Promise<void> {
	return invoke('session_retry', { sessionId });
}

export async function stopSession(sessionId: string): Promise<void> {
	return invoke('session_stop', { sessionId });
}

// Issue State
export async function getIssueStates(repoId: number): Promise<[number, string][]> {
	return invoke('get_issue_states', { repoId });
}

export async function setIssueColumn(
	repoId: number,
	issueNumber: number,
	columnId: string
): Promise<void> {
	return invoke('set_issue_column', { repoId, issueNumber, columnId });
}

export async function closeIssue(
	owner: string,
	name: string,
	issueNumber: number
): Promise<void> {
	return invoke('github_close_issue', { owner, name, issueNumber });
}

// Merge & Test
export async function mergePR(
	owner: string,
	name: string,
	pullNumber: number
): Promise<void> {
	return invoke('github_squash_merge', { owner, name, pullNumber });
}

export async function runTest(sessionId: string): Promise<void> {
	return invoke('session_run_test', { sessionId });
}

// Settings
export async function getSettings(): Promise<AppSettings> {
	return invoke('settings_get');
}

export async function updateSettings(settings: Partial<AppSettings>): Promise<void> {
	return invoke('settings_set', { settings });
}

// Prompts
export async function getPrompts(): Promise<AgentPrompt[]> {
	return invoke('prompts_get');
}

export async function updatePrompt(stage: SessionStage, promptText: string, provider: string, model: string, effort: string): Promise<void> {
	return invoke('prompts_set', { stage, promptText, provider, model, effort });
}

export async function listModels(): Promise<ModelInfo[]> {
	return invoke('list_models');
}

// Repo Path
export async function setRepoPath(repoId: number, path: string): Promise<void> {
	return invoke('set_repo_path', { repoId, path });
}

export async function getRepoPath(repoId: number): Promise<string | null> {
	return invoke('get_repo_path', { repoId });
}

// Repo Config
export async function updateRepo(repo: RepoConfig): Promise<void> {
	return invoke('github_update_repo', { repo });
}

// Selected repo persistence
export async function getSelectedRepoId(): Promise<number | null> {
	return invoke('get_selected_repo_id');
}

export async function setSelectedRepoId(repoId: number): Promise<void> {
	return invoke('set_selected_repo_id', { repoId });
}

// Session visibility
export async function hideSession(sessionId: string): Promise<void> {
	return invoke('session_hide', { sessionId });
}

export async function unhideSession(sessionId: string): Promise<void> {
	return invoke('session_unhide', { sessionId });
}

export async function listHiddenSessions(repoId: number, issueNumber: number): Promise<Session[]> {
	return invoke('session_list_hidden', { repoId, issueNumber });
}

// Session Files
export async function listSessionFiles(sessionId: string): Promise<string[]> {
	return invoke('session_list_files', { sessionId });
}

// Session Logs
export async function fetchSessionLogs(sessionId: string): Promise<SessionLogEntry[]> {
	return invoke('session_get_logs', { sessionId });
}

// Polling
export async function startPolling(): Promise<void> {
	return invoke('start_polling');
}
