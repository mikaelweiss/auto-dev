import { writable, derived } from 'svelte/store';
import type { Issue, ColumnId } from '$lib/types';
import { getColumnForIssue, COLUMN_ORDER } from '$lib/types';
import { selectedRepoId, repos } from './repos';

export const issues = writable<Issue[]>([]);

export const issuesByColumn = derived(
	[issues, selectedRepoId, repos],
	([$issues, $selectedRepoId, $repos]) => {
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
			const column = getColumnForIssue(issue);
			grouped[column].push(issue);
		}

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
