import { writable, derived, get } from 'svelte/store';
import type { Issue, ColumnId } from '$lib/types';
import { getColumnForSession, COLUMN_ORDER } from '$lib/types';
import { selectedRepoId, repos } from './repos';
import { sessionByIssue } from './sessions';

export const issues = writable<Issue[]>([]);

/** Tracks the next page to fetch per repo (key: "owner/name") */
const nextPage = new Map<string, number>();

/** Whether more pages are available per repo (key: "owner/name") */
const repoHasMore = writable<Map<string, boolean>>(new Map());

export const hasMoreIssues = derived(
	[selectedRepoId, repos, repoHasMore],
	([$selectedRepoId, $repos, $repoHasMore]) => {
		if (!$selectedRepoId) return false;
		const repo = $repos.find((r) => r.id === $selectedRepoId);
		if (!repo) return false;
		return $repoHasMore.get(`${repo.owner}/${repo.name}`) ?? false;
	}
);

/** Local column assignments keyed by "repoId:issueNumber" */
export const issueStates = writable<Map<string, ColumnId>>(new Map());

export const issuesByColumn = derived(
	[issues, selectedRepoId, repos, sessionByIssue, issueStates],
	([$issues, $selectedRepoId, $repos, $sessionByIssue, $issueStates]) => {
		const grouped: Record<ColumnId, Issue[]> = {
			backlog: [],
			planning: [],
			in_progress: [],
			blocked: [],
			review: [],
			done: []
		};

		if (!$selectedRepoId) return grouped;

		const repo = $repos.find((r) => r.id === $selectedRepoId);
		if (!repo) return grouped;

		const filtered = $issues.filter(
			(i) => i.repo_owner === repo.owner && i.repo_name === repo.name
		);

		for (const issue of filtered) {
			// Closed issues always go to done
			if (issue.state === 'closed') {
				grouped['done'].push(issue);
				continue;
			}

			const sessionKey = `${repo.id}:${issue.number}`;
			const session = $sessionByIssue.get(sessionKey);

			// Only derive column from session if the session is actively running.
			// For completed/failed sessions, use the DB state (which gets updated by
			// state advancement signals from the AI).
			const isActive =
				session &&
				(session.status === 'running' ||
					session.status === 'initializing' ||
					session.status === 'setup');

			if (isActive) {
				const col = getColumnForSession(session);
				grouped[col].push(issue);
			} else {
				// Fall back to local DB state, default to backlog
				const col = $issueStates.get(sessionKey) ?? 'backlog';
				grouped[col].push(issue);
			}
		}

		// Sort each column by updated_at descending
		for (const col of COLUMN_ORDER) {
			grouped[col].sort(
				(a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
			);
		}

		return grouped;
	}
);

/** Merge fetched issues into the store (update existing, append new). */
function mergeIssues(fetched: Issue[]) {
	issues.update((current) => {
		const updated = [...current];
		for (const incoming of fetched) {
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

/** Refresh page 1 for a repo, replacing all issues for that repo with the fresh set. */
export async function refreshIssues(owner: string, name: string) {
	const { fetchIssues } = await import('./backend');
	const fetched = await fetchIssues(owner, name, 1);
	const key = `${owner}/${name}`;
	nextPage.set(key, 2);
	repoHasMore.update((m) => new Map(m).set(key, fetched.length >= 100));

	// Replace all issues for this repo (removes stale/closed/deleted issues)
	issues.update((current) => {
		const other = current.filter(
			(i) => !(i.repo_owner === owner && i.repo_name === name)
		);
		return [...other, ...fetched];
	});
}

/** Load the next page of issues for the currently selected repo. */
let _loadingMore = false;
export async function loadMoreIssues() {
	if (_loadingMore) return;
	_loadingMore = true;
	try {
		const $repos = get(repos);
		const $selectedRepoId = get(selectedRepoId);
		if (!$selectedRepoId) return;

		const repo = $repos.find((r) => r.id === $selectedRepoId);
		if (!repo) return;

		const key = `${repo.owner}/${repo.name}`;
		if (!get(repoHasMore).get(key)) return;

		const page = nextPage.get(key) ?? 2;
		const { fetchIssues } = await import('./backend');
		const fetched = await fetchIssues(repo.owner, repo.name, page);

		nextPage.set(key, page + 1);
		repoHasMore.update((m) => new Map(m).set(key, fetched.length >= 100));

		if (fetched.length === 0) return;

		mergeIssues(fetched);
	} finally {
		_loadingMore = false;
	}
}
