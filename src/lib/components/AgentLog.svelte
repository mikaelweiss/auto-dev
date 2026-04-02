<script lang="ts">
	import { sessionLogs } from '$lib/stores/sessions';
	import { fetchSessionLogs } from '$lib/stores/backend';
	import type { SessionLogEntry } from '$lib/types';
	import { Wrench, AlertCircle, ChevronRight, ChevronDown, Terminal } from 'lucide-svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let { sessionId }: { sessionId: string } = $props();

	let container: HTMLDivElement | undefined = $state(undefined);

	const NOISE_TYPES = new Set(['user', 'rate_limit_event', 'system']);
	let logs = $derived(
		($sessionLogs.get(sessionId) ?? []).filter((e) => !NOISE_TYPES.has(e.event_type))
	);

	$effect(() => {
		// Load persisted logs from the DB whenever sessionId changes
		const id = sessionId;
		fetchSessionLogs(id).then((entries) => {
			if (entries.length > 0) {
				sessionLogs.update((current) => {
					// Only set if the store doesn't already have fresher data (from live events)
					const existing = current.get(id);
					if (!existing || existing.length === 0) {
						current.set(id, entries);
						return new Map(current);
					}
					return current;
				});
			}
		});
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

	/** Extract tool name from content like "Bash: some command" -> "Bash" */
	function toolName(content: string): string {
		const idx = content.indexOf(':');
		if (idx > 0 && idx < 30) {
			return content.substring(0, idx).trim();
		}
		return content.trim();
	}

	type LogGroup =
		| { kind: 'message'; entry: SessionLogEntry }
		| { kind: 'error'; entry: SessionLogEntry }
		| { kind: 'status_change'; entry: SessionLogEntry }
		| { kind: 'test_output'; entry: SessionLogEntry }
		| { kind: 'tool_calls'; entries: SessionLogEntry[] };

	let groups = $derived.by((): LogGroup[] => {
		const result: LogGroup[] = [];
		let i = 0;
		while (i < logs.length) {
			const entry = logs[i];
			if (entry.event_type === 'tool_call') {
				const batch: SessionLogEntry[] = [entry];
				i++;
				while (i < logs.length && logs[i].event_type === 'tool_call') {
					batch.push(logs[i]);
					i++;
				}
				result.push({ kind: 'tool_calls', entries: batch });
			} else if (entry.event_type === 'message') {
				result.push({ kind: 'message', entry });
				i++;
			} else if (entry.event_type === 'error') {
				result.push({ kind: 'error', entry });
				i++;
			} else if (entry.event_type === 'status_change') {
				result.push({ kind: 'status_change', entry });
				i++;
			} else if (entry.event_type === 'test_output') {
				result.push({ kind: 'test_output', entry });
				i++;
			} else {
				result.push({ kind: 'message', entry });
				i++;
			}
		}
		return result;
	});

	let expandedGroups = new SvelteSet<number>();

	function toggleGroup(index: number) {
		if (expandedGroups.has(index)) {
			expandedGroups.delete(index);
		} else {
			expandedGroups.add(index);
		}
	}
</script>

<div bind:this={container} class="flex-1 overflow-y-auto pr-1 space-y-3 py-2">
	{#if logs.length === 0}
		<div class="flex items-center justify-center h-full">
			<p class="text-sm text-muted-foreground">No activity yet.</p>
		</div>
	{:else}
		{#each groups as group, groupIndex (groupIndex)}
			{#if group.kind === 'message'}
				<!-- Message: clean readable text -->
				<div class="px-2">
					<p class="text-sm text-foreground/90 leading-relaxed whitespace-pre-wrap">{group.entry.content}</p>
					<span class="text-[10px] text-muted-foreground/60 mt-1 block">{formatTime(group.entry.timestamp)}</span>
				</div>

			{:else if group.kind === 'tool_calls'}
				<!-- Tool calls: collapsible group -->
				{#if group.entries.length === 1}
					<!-- Single tool call: inline pill -->
					<div class="px-2">
						<span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-muted/60 text-xs text-muted-foreground">
							<Wrench class="h-3 w-3" />
							{toolName(group.entries[0].content)}
						</span>
					</div>
				{:else}
					<!-- Multiple tool calls: collapsible -->
					<div class="px-2">
						<button
							type="button"
							class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-muted/60 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
							onclick={() => toggleGroup(groupIndex)}
						>
							{#if expandedGroups.has(groupIndex)}
								<ChevronDown class="h-3 w-3" />
							{:else}
								<ChevronRight class="h-3 w-3" />
							{/if}
							<Wrench class="h-3 w-3" />
							Used {group.entries.length} tools
						</button>
						{#if expandedGroups.has(groupIndex)}
							<div class="mt-1.5 ml-2 pl-3 border-l-2 border-border space-y-0.5">
								{#each group.entries as toolEntry (toolEntry.id)}
									<div class="text-xs text-muted-foreground py-0.5 flex items-center gap-1.5">
										<Wrench class="h-2.5 w-2.5 shrink-0" />
										<span class="font-medium text-foreground/70">{toolName(toolEntry.content)}</span>
										<span class="text-muted-foreground/50 truncate">{toolEntry.content.substring(toolEntry.content.indexOf(':') + 1).trim()}</span>
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{/if}

			{:else if group.kind === 'error'}
				<!-- Error: compact red alert -->
				<div class="mx-2 flex items-start gap-2 px-3 py-2 rounded-md bg-destructive/10 border border-red-500/20">
					<AlertCircle class="h-4 w-4 text-red-400 shrink-0 mt-0.5" />
					<div class="min-w-0">
						<p class="text-sm text-red-400 break-words">{group.entry.content}</p>
						<span class="text-[10px] text-red-400/50 mt-0.5 block">{formatTime(group.entry.timestamp)}</span>
					</div>
				</div>

			{:else if group.kind === 'status_change'}
				<!-- Status change: centered divider -->
				<div class="flex items-center gap-3 px-2 py-1">
					<div class="flex-1 h-px bg-border"></div>
					<span class="text-[10px] text-muted-foreground/60 uppercase tracking-wider whitespace-nowrap">{group.entry.content}</span>
					<div class="flex-1 h-px bg-border"></div>
				</div>

			{:else if group.kind === 'test_output'}
				<!-- Test output: monospace code block -->
				<div class="mx-2">
					<div class="flex items-center gap-1.5 mb-1">
						<Terminal class="h-3 w-3 text-muted-foreground" />
						<span class="text-[10px] text-muted-foreground uppercase tracking-wider">Test Output</span>
					</div>
					<pre class="text-xs font-mono bg-muted/50 rounded-md p-3 overflow-x-auto text-foreground/80 whitespace-pre-wrap break-words border border-border/50">{group.entry.content}</pre>
				</div>
			{/if}
		{/each}
	{/if}
</div>
