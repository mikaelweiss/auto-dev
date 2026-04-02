import { writable, derived } from 'svelte/store';
import type { Issue, ColumnId } from '$lib/types';
import { getColumnForIssue, getColumnForSession, COLUMN_ORDER } from '$lib/types';
import { selectedRepoId, repos } from './repos';
import { sessionByIssue } from './sessions';

export const issues = writable<Issue[]>([]);

export const issuesByColumn = derived(
	[issues, selectedRepoId, repos, sessionByIssue],
	([$issues, $selectedRepoId, $repos, $sessionByIssue]) => {
		const grouped: Record<ColumnId, Issue[]> = {
			backlog: [],
			claimed: [],
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

export async function refreshIssues(owner: string, name: string) {
	const { fetchIssues } = await import('./backend');
	const fetched = await fetchIssues(owner, name);
	issues.update((current) => {
		// Remove existing issues for this repo, then add the fresh ones
		const other = current.filter(
			(i) => !(i.repo_owner === owner && i.repo_name === name)
		);
		return [...other, ...fetched];
	});
}
