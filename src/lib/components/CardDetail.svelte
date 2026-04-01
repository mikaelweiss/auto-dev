<script lang="ts">
	import { selectedIssue } from '$lib/stores/ui';
	import { sessionByIssue, sessions } from '$lib/stores/sessions';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import { getColumnForIssue } from '$lib/types';
	import { X, ExternalLink, Play, GitMerge, Send } from 'lucide-svelte';
	import AgentLog from './AgentLog.svelte';

	let issue = $derived($selectedIssue);
	let visible = $derived(issue !== null);

	let repoConfig = $derived(
		issue ? $repos.find((r) => r.owner === issue!.repo_owner && r.name === issue!.repo_name) : null
	);
	let sessionKey = $derived(repoConfig && issue ? `${repoConfig.id}:${issue.number}` : null);
	let session = $derived(sessionKey ? $sessionByIssue.get(sessionKey) ?? null : null);
	let column = $derived(issue ? getColumnForIssue(issue) : null);

	let editingTitle = $state(false);
	let titleDraft = $state('');
	let editingBody = $state(false);
	let bodyDraft = $state('');
	let blockResponse = $state('');

	// Reset editing state when issue changes
	$effect(() => {
		if (issue) {
			editingTitle = false;
			editingBody = false;
			titleDraft = issue.title;
			bodyDraft = issue.body;
			blockResponse = '';
		}
	});

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

	function handleTest() {
		if (session) {
			backend.runTest(session.id);
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
		if (session && blockResponse.trim()) {
			backend.respondToSession(session.id, blockResponse.trim());
			blockResponse = '';
		}
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

				<!-- Session info -->
				{#if session}
					<div class="space-y-2">
						<h3 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Session</h3>
						<div class="bg-muted rounded-lg p-3 space-y-1.5">
							<div class="flex items-center justify-between">
								<span class="text-sm text-foreground">Stage: <span class="font-medium">{session.stage}</span></span>
								<span class="text-xs px-2 py-0.5 rounded-full {session.status === 'running' ? 'bg-green-500/20 text-green-400' : session.status === 'failed' ? 'bg-red-500/20 text-red-400' : 'bg-muted-foreground/20 text-muted-foreground'}">
									{session.status}
								</span>
							</div>
							<div class="text-xs text-muted-foreground">
								Elapsed: {elapsedTime(session.started_at, session.completed_at)}
							</div>
							{#if session.error_message}
								<div class="text-sm text-red-400 bg-red-500/10 rounded p-2 mt-1">
									{session.error_message}
								</div>
							{/if}
						</div>
					</div>
				{/if}

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

				<!-- Blocked response -->
				{#if column === 'blocked' && session}
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
				{#if column === 'review' && session}
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
				{#if session}
					<div class="space-y-2 flex flex-col min-h-[200px]">
						<h3 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Activity</h3>
						<AgentLog sessionId={session.id} />
					</div>
				{/if}
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
