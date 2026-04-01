import { writable, derived } from 'svelte/store';
import type { Issue, ColumnId } from '$lib/types';
import { getColumnForIssue, COLUMN_ORDER } from '$lib/types';

export const issues = writable<Issue[]>([]);

export const issuesByColumn = derived(issues, ($issues) => {
	const grouped: Record<ColumnId, Issue[]> = {
		backlog: [],
		claimed: [],
		in_progress: [],
		blocked: [],
		review: [],
		done: []
	};

	for (const issue of $issues) {
		const column = getColumnForIssue(issue);
		grouped[column].push(issue);
	}

	// Sort each column by updated_at descending
	for (const col of COLUMN_ORDER) {
		grouped[col].sort(
			(a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
		);
	}

	return grouped;
});

export async function refreshIssues(owner: string, name: string) {
	const { fetchIssues } = await import('./backend');
	const fetched = await fetchIssues(owner, name);
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
