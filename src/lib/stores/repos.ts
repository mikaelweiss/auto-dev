import { writable } from 'svelte/store';
import type { RepoConfig } from '$lib/types';
import * as backend from './backend';

export const repos = writable<RepoConfig[]>([]);
export const selectedRepoId = writable<number | null>(null);

export async function selectRepo(id: number) {
	selectedRepoId.set(id);
	await backend.setSelectedRepoId(id);
	backend.loadIssueStates(id);
}

export async function removeRepo(id: number) {
	await backend.removeRepo(id);
	repos.update((list) => list.filter((r) => r.id !== id));
	// If the removed repo was selected, select another or clear
	selectedRepoId.update((current) => {
		if (current === id) return null;
		return current;
	});
}

export async function loadRepos(): Promise<RepoConfig[]> {
	const list = await backend.getRepos();
	repos.set(list);
	if (list.length > 0) {
		const savedId = await backend.getSelectedRepoId();
		const valid = savedId != null && list.some((r) => r.id === savedId);
		const repoId = valid ? savedId! : list[0].id;
		selectedRepoId.set(repoId);
		backend.loadIssueStates(repoId);
	}
	return list;
}
