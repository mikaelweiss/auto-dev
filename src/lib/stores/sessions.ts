import { writable, derived } from 'svelte/store';
import type { Session, SessionLogEntry } from '$lib/types';

export const sessions = writable<Session[]>([]);
export const sessionLogs = writable<Map<string, SessionLogEntry[]>>(new Map());

export const sessionByIssue = derived(sessions, ($sessions) => {
	const map = new Map<string, Session>();
	for (const session of $sessions) {
		const key = `${session.repo_id}:${session.issue_number}`;
		const existing = map.get(key);
		// Keep the most recent session per issue
		if (!existing || new Date(session.started_at) > new Date(existing.started_at)) {
			map.set(key, session);
		}
	}
	return map;
});
