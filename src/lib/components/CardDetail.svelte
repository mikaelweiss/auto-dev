<script lang="ts">
	import { selectedIssue } from '$lib/stores/ui';
	import { sessions, sessionsByIssue, sessionLogs } from '$lib/stores/sessions';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import type { Session, SessionStage } from '$lib/types';
	import { X, ExternalLink, Plus, Info, History, Loader2, FileText, Code, Eye } from 'lucide-svelte';
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
	let pendingNewTab = $state(false);
	let showDetails = $state(false);
	let editingBody = $state(false);
	let bodyDraft = $state('');
	let confirmHideSessionId: string | null = $state(null);
	let showHistory = $state(false);
	let hiddenSessions: Session[] = $state([]);
	let historyLoading = $state(false);
	let startingSession = $state(false);

	// Auto-select the most recent session, or follow new sessions as they appear
	let prevSessionCount = 0;
	$effect(() => {
		if (pendingNewTab) {
			// Don't auto-select when user has a pending new tab open
			prevSessionCount = allSessions.length;
			return;
		}
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
			pendingNewTab = false;
			showDetails = false;
			editingBody = false;
			bodyDraft = issue.body;
			showHistory = false;
			confirmHideSessionId = null;
			hiddenSessions = [];
			startingSession = false;
		}
	});

	let activeSession: Session | null = $derived(
		allSessions.find((s) => s.id === selectedSessionId) ?? null
	);

	// Ticking clock for live elapsed time on active sessions
	let now = $state(Date.now());
	let hasActiveSession = $derived(
		allSessions.some((s) => s.status === 'running' || s.status === 'initializing' || s.status === 'setup')
	);
	$effect(() => {
		if (!hasActiveSession) return;
		const interval = setInterval(() => { now = Date.now(); }, 1000);
		return () => clearInterval(interval);
	});

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

	function handleHideSession(sessionId: string) {
		const sess = allSessions.find((s) => s.id === sessionId);
		if (
			sess &&
			(sess.status === 'running' || sess.status === 'initializing' || sess.status === 'setup')
		) {
			confirmHideSessionId = sessionId;
			return;
		}
		backend.hideSession(sessionId);
		sessions.update((all) => all.map((s) => (s.id === sessionId ? { ...s, hidden: true } : s)));
		if (selectedSessionId === sessionId) {
			const remaining = allSessions.filter((s) => s.id !== sessionId);
			selectedSessionId = remaining.length > 0 ? remaining[0].id : null;
		}
	}

	async function confirmHideRunningSession() {
		if (!confirmHideSessionId) return;
		const idToHide = confirmHideSessionId;
		await backend.stopSession(idToHide);
		await backend.hideSession(idToHide);
		sessions.update((all) => all.map((s) => (s.id === idToHide ? { ...s, hidden: true } : s)));
		confirmHideSessionId = null;
		if (selectedSessionId === idToHide) {
			const remaining = allSessions.filter((s) => s.id !== idToHide);
			selectedSessionId = remaining.length > 0 ? remaining[0].id : null;
		}
	}

	async function handleRestoreSession(sessionId: string) {
		await backend.unhideSession(sessionId);
		sessions.update((all) => all.map((s) => (s.id === sessionId ? { ...s, hidden: false } : s)));
		hiddenSessions = hiddenSessions.filter((s) => s.id !== sessionId);
		selectedSessionId = sessionId;
	}

	async function toggleHistory() {
		showHistory = !showHistory;
		if (showHistory && repoConfig && issue) {
			historyLoading = true;
			try {
				hiddenSessions = await backend.listHiddenSessions(repoConfig.id, issue.number);
			} catch {
				hiddenSessions = [];
			}
			historyLoading = false;
		}
	}

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
		const end = completedAt ? new Date(completedAt).getTime() : now;
		const diffMs = end - start;
		const seconds = Math.floor(diffMs / 1000);
		if (seconds < 60) return `${seconds}s`;
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ${seconds % 60}s`;
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

	function handleNewTab() {
		pendingNewTab = true;
		selectedSessionId = null;
	}

	function handleStartPhase(stage: SessionStage) {
		if (!repoConfig || !issue) return;
		pendingNewTab = false;
		startingSession = true;
		let promise: Promise<void>;
		switch (stage) {
			case 'spec':
				promise = backend.startSession(repoConfig.id, issue.number);
				break;
			case 'implement':
				promise = backend.startImplementSession(repoConfig.id, issue.number);
				break;
			case 'review':
				promise = backend.startReviewSession(repoConfig.id, issue.number);
				break;
			default:
				startingSession = false;
				return;
		}
		promise.finally(() => { startingSession = false; });
	}

	function handleNewSession() {
		handleStartPhase('spec');
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
				<div class="shrink-0 border-b border-border">
					<div class="px-4 py-2 flex items-center gap-2">
						<div class="flex gap-1 flex-1 overflow-x-auto">
							{#each allSessions as sess (sess.id)}
								<button
									class="group flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium whitespace-nowrap transition-colors
										{sess.id === selectedSessionId && !pendingNewTab
										? 'bg-muted text-foreground'
										: 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
									onclick={() => {
										selectedSessionId = sess.id;
										pendingNewTab = false;
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
									<span
										role="button"
										tabindex="0"
										class="ml-0.5 p-0.5 rounded hover:bg-background/50 opacity-0 group-hover:opacity-100 transition-opacity"
										onclick={(e) => { e.stopPropagation(); handleHideSession(sess.id); }}
										onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); handleHideSession(sess.id); } }}
										title="Close tab"
									>
										<X class="h-3 w-3" />
									</span>
								</button>
							{/each}
							{#if pendingNewTab}
								<button
									class="group flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium whitespace-nowrap transition-colors bg-muted text-foreground"
								>
									<span class="h-2 w-2 rounded-full shrink-0 bg-muted-foreground"></span>
									new
									<span
										role="button"
										tabindex="0"
										class="ml-0.5 p-0.5 rounded hover:bg-background/50 opacity-0 group-hover:opacity-100 transition-opacity"
										onclick={(e) => { e.stopPropagation(); pendingNewTab = false; if (allSessions.length > 0) selectedSessionId = allSessions[0].id; }}
										onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); pendingNewTab = false; if (allSessions.length > 0) selectedSessionId = allSessions[0].id; } }}
										title="Close tab"
									>
										<X class="h-3 w-3" />
									</span>
								</button>
							{/if}
						</div>
						<button
							class="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
							onclick={handleNewTab}
							disabled={startingSession}
							title="New session"
						>
							{#if startingSession}
								<Loader2 class="h-3.5 w-3.5 animate-spin" />
							{:else}
								<Plus class="h-3.5 w-3.5" />
							{/if}
						</button>
						<div class="relative">
							<button
								class="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors shrink-0 {showHistory ? 'bg-muted text-foreground' : ''}"
								onclick={toggleHistory}
								title="Session history"
							>
								<History class="h-3.5 w-3.5" />
							</button>

							<!-- History popover -->
							{#if showHistory}
								<div class="absolute right-0 top-full mt-1 z-50 w-64 rounded-lg border border-border bg-popover shadow-lg overflow-hidden">
									{#if historyLoading}
										<div class="px-3 py-2 text-xs text-muted-foreground">Loading...</div>
									{:else if hiddenSessions.length === 0}
										<div class="px-3 py-2 text-xs text-muted-foreground">No closed sessions.</div>
									{:else}
										<div class="max-h-48 overflow-y-auto">
											{#each hiddenSessions as sess (sess.id)}
												<div class="flex items-center gap-2 px-3 py-1.5 hover:bg-muted/50 transition-colors">
													<span class="h-2 w-2 rounded-full shrink-0 {statusDotClass(sess.status)}"></span>
													<span class="text-xs font-medium text-foreground flex-1 truncate">{sess.stage}</span>
													<span class="text-[10px] text-muted-foreground whitespace-nowrap">{timeAgo(sess.started_at)}</span>
													<button
														class="p-0.5 rounded text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
														onclick={() => handleRestoreSession(sess.id)}
														title="Restore session"
													>
														<History class="h-3 w-3" />
													</button>
												</div>
											{/each}
										</div>
									{/if}
								</div>
							{/if}
						</div>
					</div>

					<!-- Confirmation bar for closing running sessions -->
					{#if confirmHideSessionId}
						<div class="border-t border-border bg-red-500/5 px-4 py-3 flex items-center gap-3">
							<span class="text-xs text-foreground flex-1">Stop running agent and close tab?</span>
							<button
								class="px-2.5 py-1 rounded-md text-xs font-medium bg-red-600 hover:bg-red-500 text-white transition-colors"
								onclick={confirmHideRunningSession}
							>
								Stop & close
							</button>
							<button
								class="px-2.5 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
								onclick={() => { confirmHideSessionId = null; }}
							>
								Cancel
							</button>
						</div>
					{/if}
				</div>

			<!-- Error banner -->
			{#if activeSession?.error_message}
				<div class="shrink-0 px-4 py-2">
					<div class="text-sm text-red-400 bg-red-500/10 rounded p-2">
						{activeSession.error_message}
					</div>
				</div>
			{/if}

			<!-- Agent log -->
			{#if activeSession}
				<div class="flex-1 min-h-0 flex flex-col overflow-hidden">
					<AgentLog sessionId={activeSession.id} sessionStatus={activeSession.status} sessionStage={activeSession.stage} />
				</div>
			{:else if pendingNewTab}
				<div class="flex-1 flex items-center justify-center">
					<div class="flex flex-col items-center gap-4">
						<p class="text-sm text-muted-foreground">Start a phase or send a message</p>
						<div class="flex gap-2">
							<button
								class="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-muted/50 hover:bg-muted text-sm text-foreground transition-colors"
								onclick={() => handleStartPhase('spec')}
							>
								<FileText class="h-4 w-4 text-blue-400" />
								Spec
							</button>
							<button
								class="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-muted/50 hover:bg-muted text-sm text-foreground transition-colors"
								onclick={() => handleStartPhase('implement')}
							>
								<Code class="h-4 w-4 text-green-400" />
								Implement
							</button>
							<button
								class="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-muted/50 hover:bg-muted text-sm text-foreground transition-colors"
								onclick={() => handleStartPhase('review')}
							>
								<Eye class="h-4 w-4 text-yellow-400" />
								Review
							</button>
						</div>
					</div>
				</div>
			{:else}
				<div class="flex-1 flex items-center justify-center">
					{#if startingSession}
						<div class="flex flex-col items-center gap-3">
							<Loader2 class="h-5 w-5 text-muted-foreground animate-spin" />
							<p class="text-sm text-muted-foreground">Starting session...</p>
						</div>
					{:else}
						<div class="flex flex-col items-center gap-4">
							<p class="text-sm text-muted-foreground">Start a phase or send a message</p>
							<div class="flex gap-2">
								<button
									class="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-muted/50 hover:bg-muted text-sm text-foreground transition-colors"
									onclick={() => handleStartPhase('spec')}
								>
									<FileText class="h-4 w-4 text-blue-400" />
									Spec
								</button>
								<button
									class="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-muted/50 hover:bg-muted text-sm text-foreground transition-colors"
									onclick={() => handleStartPhase('implement')}
								>
									<Code class="h-4 w-4 text-green-400" />
									Implement
								</button>
								<button
									class="flex items-center gap-2 px-4 py-2 rounded-lg border border-border bg-muted/50 hover:bg-muted text-sm text-foreground transition-colors"
									onclick={() => handleStartPhase('review')}
								>
									<Eye class="h-4 w-4 text-yellow-400" />
									Review
								</button>
							</div>
						</div>
					{/if}
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
