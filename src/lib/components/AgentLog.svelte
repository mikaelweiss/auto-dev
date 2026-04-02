<script lang="ts">
	import { sessionLogs } from '$lib/stores/sessions';
	import { fetchSessionLogs } from '$lib/stores/backend';
	import { Wrench, MessageSquare, AlertCircle, RefreshCw, Terminal } from 'lucide-svelte';

	let { sessionId }: { sessionId: string } = $props();

	let container: HTMLDivElement | undefined = $state(undefined);

	let logs = $derived(($sessionLogs.get(sessionId) ?? []).slice());

	$effect(() => {
		// Load persisted logs from the DB if none exist in the store yet
		const currentLogs = $sessionLogs.get(sessionId);
		if (!currentLogs || currentLogs.length === 0) {
			fetchSessionLogs(sessionId).then((entries) => {
				if (entries.length > 0) {
					sessionLogs.update((current) => {
						current.set(sessionId, entries);
						return new Map(current);
					});
				}
			});
		}
	});

	$effect(() => {
		// Auto-scroll when new entries arrive
		if (logs.length && container) {
			// Use a microtask so the DOM has updated
			queueMicrotask(() => {
				if (container) {
					container.scrollTop = container.scrollHeight;
				}
			});
		}
	});

	function formatTime(ts: string): string {
		const d = new Date(ts);
		return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
	}
</script>

<div bind:this={container} class="flex-1 overflow-y-auto space-y-1 pr-1">
	{#if logs.length === 0}
		<div class="flex items-center justify-center h-full">
			<p class="text-sm text-muted-foreground">No activity yet.</p>
		</div>
	{:else}
		{#each logs as entry (entry.id)}
			<div
				class="flex items-start gap-2 px-2 py-1.5 rounded-md text-sm
					{entry.event_type === 'error' ? 'bg-destructive/10 text-red-400' : ''}
					{entry.event_type === 'tool_call' ? 'bg-muted/50 font-mono text-xs' : ''}
					{entry.event_type === 'test_output' ? 'bg-muted/50 font-mono text-xs' : ''}
				"
			>
				<span class="mt-0.5 shrink-0 text-muted-foreground">
					{#if entry.event_type === 'tool_call'}
						<Wrench class="h-3.5 w-3.5" />
					{:else if entry.event_type === 'message'}
						<MessageSquare class="h-3.5 w-3.5" />
					{:else if entry.event_type === 'error'}
						<AlertCircle class="h-3.5 w-3.5 text-red-400" />
					{:else if entry.event_type === 'test_output'}
						<Terminal class="h-3.5 w-3.5" />
					{:else}
						<RefreshCw class="h-3.5 w-3.5" />
					{/if}
				</span>
				<span class="shrink-0 text-muted-foreground text-xs mt-px">{formatTime(entry.timestamp)}</span>
				<span class="break-words min-w-0">{entry.content}</span>
			</div>
		{/each}
	{/if}
</div>
