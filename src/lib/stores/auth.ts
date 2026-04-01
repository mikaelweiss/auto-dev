import { writable } from 'svelte/store';
import type { GitHubUser } from '$lib/types';
import * as backend from './backend';

export const authenticated = writable(false);
export const currentUser = writable<GitHubUser | null>(null);
export const authError = writable<string | null>(null);
export const authLoading = writable(false);

export async function checkAuth() {
	const status = await backend.getAuthStatus();
	authenticated.set(status.authenticated);
	currentUser.set(status.user);
}

export async function startAuth() {
	authError.set(null);
	authLoading.set(true);
	try {
		const user = await backend.authFromCli();
		authenticated.set(true);
		currentUser.set(user);
	} catch (e) {
		authError.set(String(e));
	} finally {
		authLoading.set(false);
	}
}
