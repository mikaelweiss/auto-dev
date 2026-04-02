<script lang="ts">
	import { selectedIssue } from '$lib/stores/ui';
	import { sessionsByIssue, sessionLogs } from '$lib/stores/sessions';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import { getColumnForIssue } from '$lib/types';
	import type { Session } from '$lib/types';
	import { X, ExternalLink, Play, GitMerge, Send, Plus, Copy, Check as CheckIcon } from 'lucide-svelte';
	import AgentLog from './AgentLog.svelte';

	let issue = $derived($selectedIssue);
	let visible = $derived(issue !== null);

	let repoConfig = $derived(
		issue ? $repos.find((r) => r.owner === issue!.repo_owner && r.name === issue!.repo_name) : null
	);
	let sessionKey = $derived(repoConfig && issue ? `${repoConfig.id}:${issue.number}` : null);
	let allSessions = $derived(sessionKey ? $sessionsByIssue.get(sessionKey) ?? [] : []);
	let column = $derived(issue ? getColumnForIssue(issue) : null);

	let selectedSessionId: string | null = $state(null);

	// Auto-select the most recent session, or follow new sessions as they appear
	$effect(() => {
		if (allSessions.length > 0) {
			const ids = new Set(allSessions.map((s) => s.id));
			if (!selectedSessionId || !ids.has(selectedSessionId)) {
				selectedSessionId = allSessions[0].id;
			}
		} else {
			selectedSessionId = null;
		}
	});

	// Reset when issue changes
	$effect(() => {
		if (issue) {
			selectedSessionId = null;
			editingTitle = false;
			editingBody = false;
			titleDraft = issue.title;
			bodyDraft = issue.body;
			blockResponse = '';
		}
	});

	let activeSession: Session | null = $derived(
		allSessions.find((s) => s.id === selectedSessionId) ?? null
	);

	let editingTitle = $state(false);
	let titleDraft = $state('');
	let editingBody = $state(false);
	let bodyDraft = $state('');
	let blockResponse = $state('');

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

	function handleRespond() {
		if (activeSession && blockResponse.trim()) {
			backend.respondToSession(activeSession.id, blockResponse.trim());
			blockResponse = '';
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
				parts.push(`  [${toolBatch.length} tools] ${toolBatch.map(t => t.split(':')[0].trim()).join(', ')}`);
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
			setTimeout(() => { copied = false; }, 2000);
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
			<div class="flex items-start justify-between p-4 border-b border-border">
				<div class="flex-1 min-w-0 pr-4">
					{#if editingTitle}
						<input
							class="w-full text-lg font-semibold bg-transparent border-b border-ring outline-none text-foreground"
							bind:value={titleDraft}
							onblur={() => { editingTitle = false; }}
							onkeydown={(e) => { if (e.key === 'Enter') editingTitle = false; if (e.key === 'Escape') { titleDraft = issue.title; editingTitle = false; } }}
						/>
					{:else}
						<button
							type="button"
							class="text-lg font-semibold text-foreground cursor-text hover:bg-muted/50 rounded px-1 -mx-1 text-left bg-transparent border-none outline-none w-full"
							onclick={() => { editingTitle = true; titleDraft = issue.title; }}
						>
							{issue.title}
						</button>
					{/if}
					<div class="flex items-center gap-2 mt-1">
						<span class="text-sm text-muted-foreground">#{issue.number}</span>
						<span class="text-sm text-muted-foreground">{issue.repo_owner}/{issue.repo_name}</span>
					</div>
				</div>
				<div class="flex items-center gap-1">
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

			<!-- Content -->
			<div class="flex-1 overflow-y-auto p-4 space-y-5 min-h-0">
				<!-- Assignee -->
				<div class="flex items-center gap-3">
					<span class="text-xs font-medium uppercase tracking-wider text-muted-foreground w-20">Assignee</span>
					{#if issue.assignee}
						<div class="flex items-center gap-2">
							<img src={issue.assignee.avatar_url} alt={issue.assignee.login} class="h-6 w-6 rounded-full ring-1 ring-border" />
							<span class="text-sm text-foreground">{issue.assignee.login}</span>
						</div>
					{:else}
						<span class="text-sm text-muted-foreground italic">Unassigned</span>
					{/if}
				</div>

				<!-- Body -->
				<div class="space-y-2">
					<div class="flex items-center justify-between">
						<h3 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Description</h3>
						<button
							class="text-xs text-muted-foreground hover:text-foreground transition-colors"
							onclick={() => { editingBody = !editingBody; bodyDraft = issue.body; }}
						>
							{editingBody ? 'Cancel' : 'Edit'}
						</button>
					</div>
					{#if editingBody}
						<textarea
							class="w-full h-40 bg-muted rounded-lg p-3 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-none"
							bind:value={bodyDraft}
						></textarea>
					{:else}
						<div class="text-sm text-foreground/90 whitespace-pre-wrap bg-muted/40 rounded-lg p-3 min-h-[4rem]">
							{issue.body || 'No description provided.'}
						</div>
					{/if}
				</div>

				<!-- Session tabs -->
				<div class="space-y-2 flex flex-col flex-1 min-h-[200px]">
					<div class="flex items-center justify-between">
						<h3 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Sessions</h3>
						<button
							class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
							onclick={handleNewSession}
							title="Start new session"
						>
							<Plus class="h-3.5 w-3.5" />
							New
						</button>
					</div>

					{#if allSessions.length > 0}
						<!-- Tab bar -->
						<div class="flex gap-1 overflow-x-auto pb-1">
							{#each allSessions as sess, i (sess.id)}
								<button
									class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium whitespace-nowrap transition-colors
										{sess.id === selectedSessionId
											? 'bg-muted text-foreground'
											: 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
									onclick={() => { selectedSessionId = sess.id; }}
								>
									<span class="h-2 w-2 rounded-full shrink-0 {statusDotClass(sess.status)}"></span>
									{sess.stage}{allSessions.filter((s) => s.stage === sess.stage).length > 1
										? ` #${allSessions.filter((s) => s.stage === sess.stage).indexOf(sess) + 1}`
										: ''}
								</button>
							{/each}
						</div>

						<!-- Active session detail -->
						{#if activeSession}
							<div class="bg-muted rounded-lg p-3 space-y-1.5">
								<div class="flex items-center justify-between">
									<span class="text-sm text-foreground">Stage: <span class="font-medium">{activeSession.stage}</span></span>
									<div class="flex items-center gap-1.5">
										<button
											class="p-1 rounded hover:bg-background/50 text-muted-foreground hover:text-foreground transition-colors"
											onclick={handleCopyConversation}
											title="Copy conversation"
										>
											{#if copied}
												<CheckIcon class="h-3.5 w-3.5 text-green-400" />
											{:else}
												<Copy class="h-3.5 w-3.5" />
											{/if}
										</button>
										<span class="text-xs px-2 py-0.5 rounded-full {activeSession.status === 'running' ? 'bg-green-500/20 text-green-400' : activeSession.status === 'failed' ? 'bg-red-500/20 text-red-400' : 'bg-muted-foreground/20 text-muted-foreground'}">
											{activeSession.status}
										</span>
									</div>
								</div>
								<div class="text-xs text-muted-foreground">
									Elapsed: {elapsedTime(activeSession.started_at, activeSession.completed_at)}
								</div>
								{#if activeSession.error_message}
									<div class="text-sm text-red-400 bg-red-500/10 rounded p-2 mt-1">
										{activeSession.error_message}
									</div>
								{/if}
							</div>

							<!-- Blocked response -->
							{#if column === 'blocked'}
								<div class="space-y-2">
									<h3 class="text-xs font-medium uppercase tracking-wider text-orange-400">Blocked - Respond</h3>
									<div class="flex gap-2">
										<input
											class="flex-1 bg-muted rounded-lg px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
											placeholder="Type a response..."
											bind:value={blockResponse}
											onkeydown={(e) => { if (e.key === 'Enter') handleRespond(); }}
										/>
										<button
											class="px-3 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
											onclick={handleRespond}
											disabled={!blockResponse.trim()}
										>
											<Send class="h-4 w-4" />
										</button>
									</div>
								</div>
							{/if}

							<!-- Review actions -->
							{#if column === 'review'}
								<div class="space-y-2">
									<h3 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Actions</h3>
									<div class="flex gap-2">
										<button
											class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-green-600 hover:bg-green-500 text-white text-sm font-medium transition-colors"
											onclick={handleTest}
										>
											<Play class="h-4 w-4" />
											Test
										</button>
										{#if issue.pull_request}
											<button
												class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-purple-600 hover:bg-purple-500 text-white text-sm font-medium transition-colors"
												onclick={handleMerge}
											>
												<GitMerge class="h-4 w-4" />
												Merge
											</button>
										{/if}
									</div>
								</div>
							{/if}

							<!-- Activity log -->
							<AgentLog sessionId={activeSession.id} />
						{/if}
					{:else}
						<div class="flex items-center justify-center h-full">
							<p class="text-sm text-muted-foreground">No sessions yet.</p>
						</div>
					{/if}
				</div>
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
