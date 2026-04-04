export type ColumnId = 'backlog' | 'planning' | 'in_progress' | 'blocked' | 'review' | 'done';

export type SessionStage = 'spec' | 'implement' | 'review' | 'ci_fix' | 'merge_conflict';

export type SessionStatus = 'initializing' | 'setup' | 'running' | 'completed' | 'failed';

export interface GitHubUser {
	login: string;
	avatar_url: string;
	id: number;
}

export interface GitHubLabel {
	name: string;
	color: string;
}

export interface Issue {
	id: number;
	number: number;
	title: string;
	body: string;
	state: 'open' | 'closed';
	assignee: GitHubUser | null;
	labels: GitHubLabel[];
	created_at: string;
	updated_at: string;
	pull_request?: { url: string; html_url: string };
	html_url: string;
	repo_owner: string;
	repo_name: string;
}

export interface Session {
	id: string;
	repo_id: number;
	issue_number: number;
	stage: SessionStage;
	worktree_path: string | null;
	session_id: string | null;
	status: SessionStatus;
	error_message: string | null;
	started_at: string;
	completed_at: string | null;
	hidden: boolean;
	cost_usd: number | null;
}

export interface SessionLogEntry {
	id: string;
	session_id: string;
	timestamp: string;
	event_type:
		| 'tool_call'
		| 'message'
		| 'error'
		| 'status_change'
		| 'test_output'
		| 'thinking'
		| 'result'
		| 'rate_limit'
		| 'api_retry'
		| 'task_progress'
		| 'tool_progress';
	content: string;
}

export interface GitHubRepo {
	id: number;
	full_name: string;
	owner: { login: string };
	name: string;
	default_branch: string | null;
}

export interface RepoConfig {
	id: number;
	github_id: number;
	owner: string;
	name: string;
	full_name: string;
	setup_script: string;
	run_script: string;
	base_branch: string;
	branch_prefix: string;
}

export interface RepoRemovalInfo {
	repo_name: string;
	local_path: string | null;
	worktree_paths: string[];
	session_count: number;
	log_count: number;
}

export type AgentModel = 'haiku' | 'sonnet' | 'opus';
export type AgentEffort = 'low' | 'medium' | 'high' | 'max';

export interface AgentPrompt {
	stage: SessionStage;
	prompt_text: string;
	is_default: boolean;
	model: AgentModel;
	effort: AgentEffort;
}

export interface AppSettings {
	sleep_prevention: boolean;
	notifications_enabled: boolean;
	poll_interval_seconds: number;
	bypass_permissions: boolean;
}

export const COLUMN_CONFIG: Record<ColumnId, { label: string; github_label: string | null }> = {
	backlog: { label: 'Backlog', github_label: null },
	planning: { label: 'Planning', github_label: 'autodev:planning' },
	in_progress: { label: 'In Progress', github_label: 'autodev:in-progress' },
	blocked: { label: 'Blocked', github_label: 'autodev:blocked' },
	review: { label: 'Ready for Review', github_label: 'autodev:review' },
	done: { label: 'Done', github_label: null }
};

export const COLUMN_ORDER: ColumnId[] = [
	'backlog',
	'planning',
	'in_progress',
	'blocked',
	'review',
	'done'
];

/** Column from GitHub labels only — used when there's no local session. */
export function getColumnForIssue(issue: Issue): ColumnId {
	if (issue.state === 'closed') return 'done';
	const labelNames = issue.labels.map((l) => l.name);
	if (labelNames.includes('autodev:review')) return 'review';
	if (labelNames.includes('autodev:blocked')) return 'blocked';
	if (labelNames.includes('autodev:in-progress')) return 'in_progress';
	if (labelNames.includes('autodev:planning')) return 'planning';
	return 'backlog';
}

/** Column from local session state — takes priority over GitHub labels. */
export function getColumnForSession(session: Session): ColumnId {
	switch (session.stage) {
		case 'spec':
			return 'planning';
		case 'implement':
		case 'ci_fix':
			return 'in_progress';
		case 'review':
			return 'review';
		case 'merge_conflict':
			return 'blocked';
		default:
			return 'in_progress';
	}
}

