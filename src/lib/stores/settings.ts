import { writable } from 'svelte/store';
import type { AppSettings, AgentPrompt } from '$lib/types';
import * as backend from './backend';

export const appSettings = writable<AppSettings>({
	sleep_prevention: true,
	notifications_enabled: true,
	poll_interval_seconds: 15,
	bypass_permissions: false,
	agent_model: 'haiku',
	agent_effort: 'high'
});

export const agentPrompts = writable<AgentPrompt[]>([]);

export async function loadSettings() {
	const s = await backend.getSettings();
	appSettings.set(s);
}

export async function loadPrompts() {
	const p = await backend.getPrompts();
	agentPrompts.set(p);
}
