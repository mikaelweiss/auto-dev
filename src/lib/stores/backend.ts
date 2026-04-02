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

/** Send a log line to Rust stderr so it appears in the terminal. */
export function log(tag: string, message: string) {
	invoke('debug_log', { tag, message }).catch(() => {});
}

// Initialize: set up event listeners for Rust -> frontend events
export async function initBackend() {
	log('INIT', 'initBackend starting');

	// Load persisted sessions from DB so session state survives app restarts
	try {
		const persisted: Session[] = await invoke('session_list');
		log('INIT', `Loaded ${persisted.length} sessions from DB: ${persisted.map(s => `id=${s.id} repo=${s.repo_id} issue=#${s.issue_number} stage=${s.stage} status=${s.status}`).join(', ')}`);
		sessions.set(persisted);
	} catch (e) {
		log('INIT', `FAILED to load sessions: ${e}`);
	}

	await listen<{ issues: Issue[]; repo_owner: string; repo_name: string }>(
		'issues-updated',
		(event) => {
			log('POLL', `issues-updated: ${event.payload.repo_owner}/${event.payload.repo_name} count=${event.payload.issues.length}`);
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
		const s = event.payload;
		log('EVENT', `session-status: id=${s.id} repo=${s.repo_id} issue=#${s.issue_number} stage=${s.stage} status=${s.status} error=${s.error_message ?? 'none'}`);
		sessions.update((current) => {
			const idx = current.findIndex((s) => s.id === event.payload.id);
			if (idx >= 0) {
				log('EVENT', `session-status: updating existing session at index ${idx}`);
				current[idx] = event.payload;
				return [...current];
			}
			log('EVENT', `session-status: adding new session (total=${current.length + 1})`);
			return [...current, event.payload];
		});
	});

	await listen<{ session_id: string; entry: SessionLogEntry }>('session-log', (event) => {
		const e = event.payload.entry;
		log('EVENT', `session-log: session=${event.payload.session_id} type=${e.event_type} content=${e.content.substring(0, 100)}`);
		sessionLogs.update((current) => {
			const logs = current.get(event.payload.session_id) ?? [];
			logs.push(event.payload.entry);
			current.set(event.payload.session_id, logs);
			return new Map(current);
		});
	});

	await listen<{ session_id: string; question: string }>('session-blocked', (event) => {
		log('EVENT', `session-blocked: session=${event.payload.session_id}`);
	});

	await listen<{ session_id: string; error: string }>('session-error', (event) => {
		log('EVENT', `session-error: session=${event.payload.session_id} error=${event.payload.error}`);
	});

	await listen<{ session_id: string; line: string }>('test-output', (event) => {
		log('EVENT', `test-output: session=${event.payload.session_id} line=${event.payload.line}`);
	});

	log('INIT', 'initBackend complete');
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
