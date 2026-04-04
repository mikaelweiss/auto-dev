export type ColumnId = 'backlog' | 'planning' | 'in_progress' | 'blocked' | 'review' | 'done';

export type SessionStage = 'spec' | 'implement' | 'review' | 'ci_fix' | 'merge_conflict';

export type SessionStatus = 'initializing' | 'setup' | 'running' | 'completed' | 'failed';

export type ProviderKind = 'claude' | 'codex' | 'opencode';

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
	provider: ProviderKind;
	model: string;
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
		| 'tool_progress'
		| 'user_message';
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

export interface ModelInfo {
	id: string;
	display_name: string;
	provider: ProviderKind;
	default_effort: string;
	effort_levels: string[];
}

export interface AgentPrompt {
	stage: SessionStage;
	prompt_text: string;
	is_default: boolean;
	provider: ProviderKind;
	model: string;
	effort: string;
}

/** All available models, grouped by provider. */
export const MODEL_REGISTRY: ModelInfo[] = [
	// Claude models
	{ id: 'claude-opus-4-6-max-ctx', display_name: 'Opus 4.6 1M', provider: 'claude', default_effort: 'high', effort_levels: ['low', 'medium', 'high', 'max'] },
	{ id: 'claude-opus-4-6', display_name: 'Opus 4.6', provider: 'claude', default_effort: 'high', effort_levels: ['low', 'medium', 'high', 'max'] },
	{ id: 'claude-sonnet-4-6', display_name: 'Sonnet 4.6', provider: 'claude', default_effort: 'high', effort_levels: ['low', 'medium', 'high', 'max'] },
	{ id: 'claude-haiku-4-5', display_name: 'Haiku 4.5', provider: 'claude', default_effort: 'high', effort_levels: ['low', 'medium', 'high', 'max'] },
	// Codex models
	{ id: 'gpt-5.4', display_name: 'GPT-5.4', provider: 'codex', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'gpt-5.3-codex-spark', display_name: 'GPT-5.3-Codex-Spark', provider: 'codex', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'gpt-5.3-codex', display_name: 'GPT-5.3-Codex', provider: 'codex', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'gpt-5.2-codex', display_name: 'GPT-5.2-Codex', provider: 'codex', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	// Opencode models
	{ id: 'openai/gpt-5.4', display_name: 'GPT-5.4 (Opencode)', provider: 'opencode', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'openai/gpt-5.3-codex', display_name: 'GPT-5.3-Codex (Opencode)', provider: 'opencode', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'github-copilot/claude-sonnet-4.6', display_name: 'Sonnet 4.6 (Copilot)', provider: 'opencode', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'github-copilot/claude-opus-4.6', display_name: 'Opus 4.6 (Copilot)', provider: 'opencode', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'github-copilot/gpt-5.4', display_name: 'GPT-5.4 (Copilot)', provider: 'opencode', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
	{ id: 'github-copilot/gemini-2.5-pro', display_name: 'Gemini 2.5 Pro (Copilot)', provider: 'opencode', default_effort: 'medium', effort_levels: ['low', 'medium', 'high'] },
];

export const PROVIDER_LABELS: Record<ProviderKind, string> = {
	claude: 'Claude Code',
	codex: 'Codex',
	opencode: 'Opencode',
};

export function getModelInfo(modelId: string): ModelInfo | undefined {
	return MODEL_REGISTRY.find((m) => m.id === modelId);
}

export function getModelsForProvider(provider: ProviderKind): ModelInfo[] {
	return MODEL_REGISTRY.filter((m) => m.provider === provider);
}

export function getProviderForModel(modelId: string): ProviderKind {
	return getModelInfo(modelId)?.provider ?? 'claude';
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

