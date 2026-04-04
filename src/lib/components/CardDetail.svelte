<script lang="ts">
	import { selectedIssue } from '$lib/stores/ui';
	import { sessionsByIssue, sessionLogs } from '$lib/stores/sessions';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import type { Session } from '$lib/types';
	import { X, ExternalLink, Plus, Info } from 'lucide-svelte';
	import AgentLog from './AgentLog.svelte';
	import ChatInput from './ChatInput.svelte';

	let issue = $derived($selectedIssue);
	let visible = $derived(issue !== null);

	let repoConfig = $derived(
		issue ? $repos.find((r) => r.owner === issue!.repo_owner && r.name === issue!.repo_name) : null
	);
	let sessionKey = $derived(repoConfig && issue ? `${repoConfig.id}:${issue.number}` : null);
	let allSessions = $derived(sessionKey ? $sessionsByIssue.get(sessionKey) ?? [] : []);

	let selectedSessionId: string | null = $state(null);
	let showDetails = $state(false);
	let editingBody = $state(false);
	let bodyDraft = $state('');

	// Auto-select the most recent session, or follow new sessions as they appear
	let prevSessionCount = 0;
	$effect(() => {
		if (allSessions.length > 0) {
			const ids = new Set(allSessions.map((s) => s.id));
			if (!selectedSessionId || !ids.has(selectedSessionId)) {
				selectedSessionId = allSessions[0].id;
			} else if (allSessions.length > prevSessionCount && prevSessionCount > 0) {
				const newest = allSessions[0];
				if (
					newest.status === 'running' ||
					newest.status === 'initializing' ||
					newest.status === 'setup'
				) {
					selectedSessionId = newest.id;
				}
			}
		} else {
			selectedSessionId = null;
		}
		prevSessionCount = allSessions.length;
	});

	// Reset when issue changes
	$effect(() => {
		if (issue) {
			selectedSessionId = null;
			showDetails = false;
			editingBody = false;
			bodyDraft = issue.body;
		}
	});

	let activeSession: Session | null = $derived(
		allSessions.find((s) => s.id === selectedSessionId) ?? null
	);

	function close() {
		selectedIssue.set(null);
	}

	function handleOverlayClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			close();
		}
	}

	function handleOverlayKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			close();
		}
	}

	function elapsedTime(startedAt: string, completedAt: string | null): string {
		const start = new Date(startedAt).getTime();
		const end = completedAt ? new Date(completedAt).getTime() : Date.now();
		const diffMs = end - start;
		const minutes = Math.floor(diffMs / 60000);
		if (minutes < 60) return `${minutes}m`;
		const hours = Math.floor(minutes / 60);
		const remainingMin = minutes % 60;
		return `${hours}h ${remainingMin}m`;
	}

	function statusDotClass(status: string): string {
		switch (status) {
			case 'running':
				return 'bg-green-500 animate-pulse';
			case 'initializing':
			case 'setup':
				return 'bg-yellow-500 animate-pulse';
			case 'failed':
				return 'bg-red-500';
			case 'completed':
				return 'bg-green-500';
			default:
				return 'bg-muted-foreground';
		}
	}

	function handleTest() {
		if (activeSession) {
			backend.runTest(activeSession.id);
		}
	}

	function handleMerge() {
		if (issue?.pull_request) {
			const prNumber = parseInt(issue.pull_request.url.split('/').pop() ?? '0', 10);
			if (prNumber) {
				backend.mergePR(issue.repo_owner, issue.repo_name, prNumber);
			}
		}
	}

	function handleNewSession() {
		if (repoConfig && issue) {
			backend.startSession(repoConfig.id, issue.number);
		}
	}

	let copied = $state(false);

	function handleCopyConversation() {
		if (!activeSession) return;
		const logs = $sessionLogs.get(activeSession.id) ?? [];
		const parts: string[] = [];
		let toolBatch: string[] = [];

		function flushTools() {
			if (toolBatch.length === 0) return;
			if (toolBatch.length === 1) {
				parts.push(`  [tool] ${toolBatch[0]}`);
			} else {
				parts.push(
					`  [${toolBatch.length} tools] ${toolBatch.map((t) => t.split(':')[0].trim()).join(', ')}`
				);
			}
			toolBatch = [];
		}

		for (const entry of logs) {
			if (entry.event_type === 'tool_call') {
				toolBatch.push(entry.content);
			} else {
				flushTools();
				if (entry.event_type === 'message') {
					parts.push(entry.content);
				} else if (entry.event_type === 'error') {
					parts.push(`[error] ${entry.content}`);
				} else if (entry.event_type === 'status_change') {
					parts.push(`--- ${entry.content} ---`);
				} else if (entry.event_type === 'test_output') {
					parts.push(`[test]\n${entry.content}`);
				}
			}
		}
		flushTools();

		navigator.clipboard.writeText(parts.join('\n\n')).then(() => {
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 2000);
		});
	}

	function openInBrowser() {
		if (issue) {
			window.open(issue.html_url, '_blank');
		}
	}
</script>

{#if visible && issue}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex justify-end"
		onclick={handleOverlayClick}
		onkeydown={handleOverlayKeydown}
	>
		<!-- Backdrop -->
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="absolute inset-0 bg-black/40 backdrop-blur-sm" onclick={close}></div>

		<!-- Panel -->
		<div
			class="relative w-full max-w-lg bg-background border-l border-border shadow-2xl flex flex-col h-full animate-slide-in"
		>
			<!-- Header -->
			<div class="flex items-start justify-between px-4 pt-4 pb-3 shrink-0">
				<div class="flex-1 min-w-0 pr-4">
					<h2 class="text-lg font-semibold text-foreground leading-tight">
						{issue.title}
					</h2>
					<span class="text-sm text-muted-foreground block mt-0.5">
						#{issue.number} · {issue.repo_owner}/{issue.repo_name}
					</span>
				</div>
				<div class="flex items-center gap-1">
					<button
						class="p-1.5 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors {showDetails ? 'bg-muted text-foreground' : ''}"
						onclick={() => {
							showDetails = !showDetails;
						}}
						title="Issue details"
					>
						<Info class="h-4 w-4" />
					</button>
					<button
						class="p-1.5 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
						onclick={openInBrowser}
						title="Open on GitHub"
					>
						<ExternalLink class="h-4 w-4" />
					</button>
					<button
						class="p-1.5 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
						onclick={close}
						title="Close"
					>
						<X class="h-4 w-4" />
					</button>
				</div>
			</div>

			<!-- Issue details (hidden by default) -->
			{#if showDetails}
				<div class="shrink-0 px-4 pb-3 border-b border-border space-y-3">
					<div class="flex items-center gap-3">
						<span class="text-xs text-muted-foreground w-20">Assignee</span>
						{#if issue.assignee}
							<div class="flex items-center gap-2">
								<img
									src={issue.assignee.avatar_url}
									alt={issue.assignee.login}
									class="h-5 w-5 rounded-full ring-1 ring-border"
								/>
								<span class="text-sm text-foreground">{issue.assignee.login}</span>
							</div>
						{:else}
							<span class="text-sm text-muted-foreground">Unassigned</span>
						{/if}
					</div>

					<div class="space-y-1.5">
						<div class="flex items-center justify-between">
							<span class="text-xs text-muted-foreground">Description</span>
							<button
								class="text-xs text-muted-foreground hover:text-foreground transition-colors"
								onclick={() => {
									editingBody = !editingBody;
									bodyDraft = issue.body;
								}}
							>
								{editingBody ? 'Cancel' : 'Edit'}
							</button>
						</div>
						{#if editingBody}
							<textarea
								class="w-full h-32 bg-muted rounded-lg p-3 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-none"
								bind:value={bodyDraft}
							></textarea>
						{:else}
							<div
								class="text-sm text-foreground/90 whitespace-pre-wrap bg-muted/40 rounded-lg p-3 max-h-[6rem] overflow-y-auto"
							>
								{issue.body || 'No description provided.'}
							</div>
						{/if}
					</div>
				</div>
			{/if}

			<!-- Session tabs -->
			{#if allSessions.length > 0}
				<div class="shrink-0 px-4 py-2 border-b border-border flex items-center gap-2">
					<div class="flex gap-1 flex-1 overflow-x-auto">
						{#each allSessions as sess (sess.id)}
							<button
								class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium whitespace-nowrap transition-colors
									{sess.id === selectedSessionId
									? 'bg-muted text-foreground'
									: 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
								onclick={() => {
									selectedSessionId = sess.id;
								}}
							>
								<span class="h-2 w-2 rounded-full shrink-0 {statusDotClass(sess.status)}"></span>
								{sess.stage}{allSessions.filter((s) => s.stage === sess.stage).length > 1
									? ` #${allSessions.filter((s) => s.stage === sess.stage).indexOf(sess) + 1}`
									: ''}
								{#if sess.id === selectedSessionId}
									<span class="text-muted-foreground font-normal ml-0.5">
										{elapsedTime(sess.started_at, sess.completed_at)}
									</span>
								{/if}
							</button>
						{/each}
					</div>
					<button
						class="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors shrink-0"
						onclick={handleNewSession}
						title="New session"
					>
						<Plus class="h-3.5 w-3.5" />
					</button>
				</div>
			{/if}

			<!-- Error banner -->
			{#if activeSession?.error_message}
				<div class="shrink-0 px-4 py-2">
					<div class="text-sm text-red-400 bg-red-500/10 rounded p-2">
						{activeSession.error_message}
					</div>
				</div>
			{/if}

			<!-- Agent log -->
			{#if allSessions.length > 0 && activeSession}
				<div class="flex-1 min-h-0 flex flex-col overflow-hidden">
					<AgentLog sessionId={activeSession.id} />
				</div>
			{:else}
				<div class="flex-1 flex items-center justify-center">
					<p class="text-sm text-muted-foreground">No sessions yet.</p>
				</div>
			{/if}

			<!-- Chat input -->
			<div class="shrink-0">
				<ChatInput
					session={activeSession}
					{issue}
					repoConfig={repoConfig ?? null}
					onNewSession={handleNewSession}
					onTest={handleTest}
					onMerge={handleMerge}
					onCopy={handleCopyConversation}
				/>
			</div>
		</div>
	</div>
{/if}

<style>
	.animate-slide-in {
		animation: slideIn 0.2s ease-out;
	}
	@keyframes slideIn {
		from {
			transform: translateX(100%);
		}
		to {
			transform: translateX(0);
		}
	}
</style>
