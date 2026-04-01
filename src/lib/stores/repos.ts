import { writable } from 'svelte/store';
import type { RepoConfig } from '$lib/types';
import * as backend from './backend';

export const repos = writable<RepoConfig[]>([]);
export const selectedRepoId = writable<number | null>(null);

export async function loadRepos() {
	const list = await backend.getRepos();
	repos.set(list);
}
