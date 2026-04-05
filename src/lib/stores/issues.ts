import { writable, derived, get } from 'svelte/store';
import type { Issue, ColumnId } from '$lib/types';
import { getColumnForIssue, getColumnForSession, COLUMN_ORDER } from '$lib/types';
import { selectedRepoId, repos } from './repos';
import { sessionByIssue } from './sessions';

export const issues = writable<Issue[]>([]);

/** Tracks the next page to fetch per repo (key: "owner/name") */
const nextPage = new Map<string, number>();

/** Whether more pages are available per repo (key: "owner/name") */
const repoHasMore = new Map<string, boolean>();

export const hasMoreIssues = writable(false);

export const issuesByColumn = derived(
	[issues, selectedRepoId, repos, sessionByIssue],
	([$issues, $selectedRepoId, $repos, $sessionByIssue]) => {
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

			// Check for a local session — session state is king
			const sessionKey = `${repo.id}:${issue.number}`;
			const session = $sessionByIssue.get(sessionKey);

			if (session) {
				const col = getColumnForSession(session);
				grouped[col].push(issue);
			} else {
				// No session — fall back to GitHub labels
				const col = getColumnForIssue(issue);
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

/** Refresh page 1 for a repo (replaces existing issues for that repo with page 1). */
export async function refreshIssues(owner: string, name: string) {
	const { fetchIssues } = await import('./backend');
	const fetched = await fetchIssues(owner, name, 1);
	const key = `${owner}/${name}`;
	nextPage.set(key, 2);
	repoHasMore.set(key, fetched.length >= 100);
	hasMoreIssues.set(fetched.length >= 100);

	issues.update((current) => {
		// Remove existing issues for this repo, then add the fresh ones
		const other = current.filter(
			(i) => !(i.repo_owner === owner && i.repo_name === name)
		);
		return [...other, ...fetched];
	});
}

/** Load the next page of issues for the currently selected repo. */
export async function loadMoreIssues() {
	const $repos = get(repos);
	const $selectedRepoId = get(selectedRepoId);
	if (!$selectedRepoId) return;

	const repo = $repos.find((r) => r.id === $selectedRepoId);
	if (!repo) return;

	const key = `${repo.owner}/${repo.name}`;
	if (!repoHasMore.get(key)) return;

	const page = nextPage.get(key) ?? 2;
	const { fetchIssues } = await import('./backend');
	const fetched = await fetchIssues(repo.owner, repo.name, page);

	nextPage.set(key, page + 1);
	repoHasMore.set(key, fetched.length >= 100);
	hasMoreIssues.set(fetched.length >= 100);

	if (fetched.length === 0) return;

	issues.update((current) => {
		// Merge: add new issues, update existing ones
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
