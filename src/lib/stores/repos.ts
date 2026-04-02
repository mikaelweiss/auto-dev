import { writable } from 'svelte/store';
import type { RepoConfig } from '$lib/types';
import * as backend from './backend';

export const repos = writable<RepoConfig[]>([]);
export const selectedRepoId = writable<number | null>(null);

export async function selectRepo(id: number) {
	selectedRepoId.set(id);
	await backend.setSelectedRepoId(id);
}

export async function loadRepos(): Promise<RepoConfig[]> {
	const list = await backend.getRepos();
	repos.set(list);
	if (list.length > 0) {
		const savedId = await backend.getSelectedRepoId();
		const valid = savedId != null && list.some((r) => r.id === savedId);
		selectedRepoId.set(valid ? savedId : list[0].id);
	}
	return list;
}
