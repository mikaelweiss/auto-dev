<script lang="ts">
	import { sessionLogs } from '$lib/stores/sessions';
	import { fetchSessionLogs } from '$lib/stores/backend';
	import type { SessionLogEntry } from '$lib/types';
	import { Wrench, AlertCircle, AlertTriangle, Sparkles, Activity, ChevronRight, ChevronDown, Terminal, Loader2 } from 'lucide-svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let { sessionId, sessionStatus = 'running', sessionStage = '' }: { sessionId: string; sessionStatus?: string; sessionStage?: string } = $props();

	let isActive = $derived(sessionStatus === 'running' || sessionStatus === 'initializing' || sessionStatus === 'setup');

	let container: HTMLDivElement | undefined = $state(undefined);

	const NOISE_TYPES = new Set(['user', 'system']);
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
		| { kind: 'tool_calls'; entries: SessionLogEntry[] }
		| { kind: 'thinking'; entries: SessionLogEntry[] }
		| { kind: 'result'; entry: SessionLogEntry }
		| { kind: 'rate_limit'; entry: SessionLogEntry }
		| { kind: 'api_retry'; entry: SessionLogEntry }
		| { kind: 'task_progress'; entry: SessionLogEntry };

	let groups = $derived.by((): LogGroup[] => {
		const result: LogGroup[] = [];
		let i = 0;
		while (i < logs.length) {
			const entry = logs[i];
			if (entry.event_type === 'tool_call' || entry.event_type === 'tool_progress') {
				const batch: SessionLogEntry[] = [entry];
				i++;
				while (i < logs.length && (logs[i].event_type === 'tool_call' || logs[i].event_type === 'tool_progress')) {
					batch.push(logs[i]);
					i++;
				}
				result.push({ kind: 'tool_calls', entries: batch });
			} else if (entry.event_type === 'thinking') {
				const batch: SessionLogEntry[] = [entry];
				i++;
				while (i < logs.length && logs[i].event_type === 'thinking') {
					batch.push(logs[i]);
					i++;
				}
				result.push({ kind: 'thinking', entries: batch });
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
			} else if (entry.event_type === 'result') {
				result.push({ kind: 'result', entry });
				i++;
			} else if (entry.event_type === 'rate_limit') {
				result.push({ kind: 'rate_limit', entry });
				i++;
			} else if (entry.event_type === 'api_retry') {
				result.push({ kind: 'api_retry', entry });
				i++;
			} else if (entry.event_type === 'task_progress') {
				result.push({ kind: 'task_progress', entry });
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
		<div class="flex flex-col items-center justify-center h-full gap-3">
			{#if sessionStatus === 'initializing'}
				<Loader2 class="h-5 w-5 text-muted-foreground animate-spin" />
				<p class="text-sm text-muted-foreground">Initializing session...</p>
			{:else if sessionStatus === 'setup'}
				<Loader2 class="h-5 w-5 text-muted-foreground animate-spin" />
				<p class="text-sm text-muted-foreground">Running setup script...</p>
			{:else if sessionStatus === 'running'}
				<Loader2 class="h-5 w-5 text-muted-foreground animate-spin" />
				<p class="text-sm text-muted-foreground">Starting {sessionStage || 'agent'}...</p>
			{:else}
				<p class="text-sm text-muted-foreground">No activity yet.</p>
			{/if}
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

			{:else if group.kind === 'thinking'}
				<!-- Thinking: collapsible section with purple tint -->
				<div class="px-2">
					<button
						type="button"
						class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-violet-500/10 text-xs text-violet-400 hover:bg-violet-500/20 hover:text-violet-300 transition-colors"
						onclick={() => toggleGroup(groupIndex)}
					>
						{#if expandedGroups.has(groupIndex)}
							<ChevronDown class="h-3 w-3" />
						{:else}
							<ChevronRight class="h-3 w-3" />
						{/if}
						<Sparkles class="h-3 w-3" />
						Thinking...
					</button>
					{#if expandedGroups.has(groupIndex)}
						<div class="mt-1.5 ml-2 pl-3 border-l-2 border-violet-500/30 space-y-1">
							{#each group.entries as thinkEntry (thinkEntry.id)}
								<p class="text-xs text-violet-300/80 leading-relaxed whitespace-pre-wrap">{thinkEntry.content}</p>
							{/each}
						</div>
					{/if}
				</div>

			{:else if group.kind === 'result'}
				<!-- Result: subtle monospace summary -->
				<div class="mx-2 text-xs text-muted-foreground font-mono bg-muted/30 rounded px-2.5 py-1">{group.entry.content}</div>

			{:else if group.kind === 'rate_limit'}
				<!-- Rate limit: amber warning box -->
				<div class="mx-2 flex items-start gap-2 px-3 py-2 rounded-md bg-amber-500/10 border border-amber-500/20">
					<AlertTriangle class="h-4 w-4 text-amber-400 shrink-0 mt-0.5" />
					<div class="min-w-0">
						<p class="text-sm text-amber-400 break-words">{group.entry.content}</p>
						<span class="text-[10px] text-amber-400/50 mt-0.5 block">{formatTime(group.entry.timestamp)}</span>
					</div>
				</div>

			{:else if group.kind === 'api_retry'}
				<!-- API retry: amber warning box -->
				<div class="mx-2 flex items-start gap-2 px-3 py-2 rounded-md bg-amber-500/10 border border-amber-500/20">
					<AlertTriangle class="h-4 w-4 text-amber-400 shrink-0 mt-0.5" />
					<div class="min-w-0">
						<p class="text-sm text-amber-400 break-words">{group.entry.content}</p>
						<span class="text-[10px] text-amber-400/50 mt-0.5 block">{formatTime(group.entry.timestamp)}</span>
					</div>
				</div>

			{:else if group.kind === 'task_progress'}
				<!-- Task progress: centered divider with activity icon -->
				<div class="flex items-center gap-3 px-2 py-1">
					<div class="flex-1 h-px bg-border"></div>
					<span class="inline-flex items-center gap-1.5 text-[10px] text-muted-foreground/60 uppercase tracking-wider whitespace-nowrap">
						<Activity class="h-3 w-3" />
						{group.entry.content}
					</span>
					<div class="flex-1 h-px bg-border"></div>
				</div>
			{/if}
		{/each}
		{#if isActive}
			<div class="px-2 flex items-center gap-2 py-1">
				<div class="flex items-center gap-1">
					<span class="thinking-dot h-1.5 w-1.5 rounded-full bg-muted-foreground/40"></span>
					<span class="thinking-dot h-1.5 w-1.5 rounded-full bg-muted-foreground/40" style="animation-delay: 150ms"></span>
					<span class="thinking-dot h-1.5 w-1.5 rounded-full bg-muted-foreground/40" style="animation-delay: 300ms"></span>
				</div>
				<span class="text-xs text-muted-foreground/60">
					{#if sessionStatus === 'initializing'}
						Initializing...
					{:else if sessionStatus === 'setup'}
						Running setup...
					{:else}
						Thinking...
					{/if}
				</span>
			</div>
		{/if}
	{/if}
</div>

<style>
	.thinking-dot {
		animation: thinking-bounce 1.4s infinite ease-in-out both;
	}
	@keyframes thinking-bounce {
		0%, 80%, 100% {
			transform: scale(0.6);
			opacity: 0.4;
		}
		40% {
			transform: scale(1);
			opacity: 1;
		}
	}
</style>
