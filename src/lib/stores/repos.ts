import { writable } from 'svelte/store';
import type { RepoConfig } from '$lib/types';
import * as backend from './backend';

export const repos = writable<RepoConfig[]>([]);
export const selectedRepoId = writable<number | null>(null);

export async function loadRepos(): Promise<RepoConfig[]> {
	const list = await backend.getRepos();
	repos.set(list);
	if (list.length > 0) {
		selectedRepoId.set(list[0].id);
	}
	return list;
}
