<script lang="ts">
	import type { Issue } from '$lib/types';
	import { getColumnForIssue } from '$lib/types';
	import { sessionByIssue } from '$lib/stores/sessions';
	import { repos } from '$lib/stores/repos';
	import { selectedIssue } from '$lib/stores/ui';
	import * as backend from '$lib/stores/backend';
	import { Play, AlertCircle } from 'lucide-svelte';

	let { issue }: { issue: Issue } = $props();

	let column = $derived(getColumnForIssue(issue));

	let repoConfig = $derived($repos.find((r) => r.owner === issue.repo_owner && r.name === issue.repo_name));
	let sessionKey = $derived(repoConfig ? `${repoConfig.id}:${issue.number}` : null);
	let session = $derived(sessionKey ? $sessionByIssue.get(sessionKey) ?? null : null);
	let hasError = $derived(session?.status === 'failed');

	function timeAgo(dateStr: string): string {
		const now = Date.now();
		const then = new Date(dateStr).getTime();
		const diffMs = now - then;
		const minutes = Math.floor(diffMs / 60000);
		if (minutes < 1) return 'just now';
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours}h ago`;
		const days = Math.floor(hours / 24);
		return `${days}d ago`;
	}

	function handleClick() {
		selectedIssue.set(issue);
	}

	function handleTest(e: MouseEvent) {
		e.stopPropagation();
		if (session) {
			backend.runTest(session.id);
		}
	}
</script>

<div
	class="group rounded-lg border border-border bg-card p-3 shadow-sm hover:shadow-md hover:border-ring/30 transition-all cursor-pointer"
	role="button"
	tabindex="0"
	onclick={handleClick}
	onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(); }}
>
	<div class="flex items-start justify-between gap-2 mb-1.5">
		<h4 class="text-sm font-medium text-card-foreground leading-snug line-clamp-2 flex-1">
			{issue.title}
		</h4>
		{#if column === 'blocked'}
			<span class="shrink-0 mt-0.5 h-2.5 w-2.5 rounded-full bg-orange-500 animate-pulse" title="Blocked"></span>
		{/if}
		{#if hasError}
			<span class="shrink-0 mt-0.5" title={session?.error_message ?? 'Error'}>
				<AlertCircle class="h-4 w-4 text-red-500" />
			</span>
		{/if}
	</div>

	<div class="flex items-center justify-between gap-2">
		<div class="flex items-center gap-2 min-w-0">
			<span class="text-xs text-muted-foreground">#{issue.number}</span>
			{#if session}
				<span class="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
					{session.stage}
				</span>
			{/if}
		</div>

		<div class="flex items-center gap-2 shrink-0">
			{#if column === 'review' && session}
				<button
					class="flex items-center gap-1 text-xs px-2 py-0.5 rounded bg-green-600 hover:bg-green-500 text-white transition-colors"
					onclick={handleTest}
				>
					<Play class="h-3 w-3" />
					Test
				</button>
			{/if}
			<span class="text-xs text-muted-foreground">{timeAgo(issue.updated_at)}</span>
			{#if issue.assignee}
				<img
					src={issue.assignee.avatar_url}
					alt={issue.assignee.login}
					class="h-5 w-5 rounded-full ring-1 ring-border"
				/>
			{/if}
		</div>
	</div>
</div>
