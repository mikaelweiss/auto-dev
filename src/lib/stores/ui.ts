import { writable } from 'svelte/store';
import type { Issue } from '$lib/types';

export const selectedIssue = writable<Issue | null>(null);
export const showNewIssueDialog = writable(false);
export const showSettings = writable(false);
export const showAddRepo = writable(false);
export const removeRepoId = writable<number | null>(null);
export const showCommandPalette = writable(false);
