import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
	Issue,
	Session,
	SessionLogEntry,
	GitHubUser,
	GitHubRepo,
	RepoConfig,
	AppSettings,
	AgentPrompt,
	SessionStage
} from '$lib/types';
import { issues } from './issues';
import { sessions, sessionLogs } from './sessions';

// Initialize: set up event listeners for Rust -> frontend events
export async function initBackend() {
	// Load persisted sessions from DB so session state survives app restarts
	try {
		const persisted: Session[] = await invoke('session_list');
		sessions.set(persisted);
	} catch (e) {
		console.error('Failed to load sessions:', e);
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

	await listen<{ session_id: string; question: string }>('session-blocked', (_event) => {
		// Blocked state is handled via session-status updates and UI
	});

	await listen<{ session_id: string; error: string }>('session-error', (_event) => {
		// Error state is handled via session-status updates and UI
	});

	await listen<{ session_id: string; line: string }>('test-output', (_event) => {
		// Test output can be handled via session logs or dedicated UI
	});
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

export async function removeRepo(repoId: number): Promise<void> {
	return invoke('github_remove_repo', { repoId });
}

export async function getRepos(): Promise<RepoConfig[]> {
	return invoke('github_get_repos');
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
export async function startSession(repoId: number, issueNumber: number): Promise<void> {
	return invoke('session_start', { repoId, issueNumber });
}

export async function respondToSession(sessionId: string, message: string): Promise<void> {
	return invoke('session_respond', { sessionId, message });
}

export async function retrySession(sessionId: string): Promise<void> {
	return invoke('session_retry', { sessionId });
}

export async function stopSession(sessionId: string): Promise<void> {
	return invoke('session_stop', { sessionId });
}

// Labels
export async function addLabel(
	owner: string,
	name: string,
	issueNumber: number,
	label: string
): Promise<void> {
	return invoke('github_add_label', { owner, name, issueNumber, label });
}

export async function removeLabel(
	owner: string,
	name: string,
	issueNumber: number,
	label: string
): Promise<void> {
	return invoke('github_remove_label', { owner, name, issueNumber, label });
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
	return invoke('issue_run_test', { sessionId });
}

// Settings
export async function getSettings(): Promise<AppSettings> {
	return invoke('get_settings');
}

export async function updateSettings(settings: Partial<AppSettings>): Promise<void> {
	return invoke('update_settings', { settings });
}

// Prompts
export async function getPrompts(): Promise<AgentPrompt[]> {
	return invoke('get_prompts');
}

export async function updatePrompt(stage: SessionStage, promptText: string): Promise<void> {
	return invoke('update_prompt', { stage, promptText });
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

// Polling
export async function startPolling(): Promise<void> {
	return invoke('start_polling');
}
