<script lang="ts">
	import { authenticated, currentUser, authError, authLoading } from '$lib/stores/auth';
	import { checkAuth, startAuth } from '$lib/stores/auth';
	import { loadRepos, selectedRepoId, repos, selectRepo } from '$lib/stores/repos';
	import { get } from 'svelte/store';
	import { initBackend } from '$lib/stores/backend';
	import { refreshIssues } from '$lib/stores/issues';
	import { showNewIssueDialog, showSettings, showCommandPalette, showKeyboardShortcuts, selectedIssue, showAddRepo, removeRepoId } from '$lib/stores/ui';
	import { COLUMN_ORDER } from '$lib/types';
	import KanbanBoard from '$lib/components/KanbanBoard.svelte';
	import CommandPalette from '$lib/components/CommandPalette.svelte';
	import KeyboardShortcuts from '$lib/components/KeyboardShortcuts.svelte';
	import CardDetail from '$lib/components/CardDetail.svelte';
	import RepoSelector from '$lib/components/RepoSelector.svelte';
	import NewIssueDialog from '$lib/components/NewIssueDialog.svelte';
	import SettingsDialog from '$lib/components/SettingsDialog.svelte';
	import AddRepoDialog from '$lib/components/AddRepoDialog.svelte';
	import RemoveRepoDialog from '$lib/components/RemoveRepoDialog.svelte';
	import { Settings, Plus, Loader2 } from 'lucide-svelte';
	import { onMount } from 'svelte';

	let loading = $state(true);
	let error = $state('');

	function handleGlobalKeydown(e: KeyboardEvent) {
		// Don't handle shortcuts before auth
		if (!$authenticated) return;

		const target = e.target as HTMLElement;
		const inInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

		// Cmd+K — toggle command palette
		if (e.key === 'k' && e.metaKey && !e.shiftKey) {
			e.preventDefault();
			showCommandPalette.update((v) => !v);
			return;
		}

		// Cmd+N — new issue
		if (e.key === 'n' && e.metaKey && !e.shiftKey) {
			e.preventDefault();
			showNewIssueDialog.set(true);
			return;
		}

		// Cmd+/ — toggle keyboard shortcuts
		if (e.key === '/' && e.metaKey) {
			e.preventDefault();
			showKeyboardShortcuts.update((v) => !v);
			return;
		}

		// Cmd+, — toggle settings
		if (e.key === ',' && e.metaKey) {
			e.preventDefault();
			showSettings.update((v) => !v);
			return;
		}

		// Escape — close topmost overlay
		if (e.key === 'Escape') {
			if (get(showCommandPalette)) { showCommandPalette.set(false); return; }
			if (get(showKeyboardShortcuts)) { showKeyboardShortcuts.set(false); return; }
			if (get(selectedIssue)) { selectedIssue.set(null); return; }
			if (get(showSettings)) { showSettings.set(false); return; }
			if (get(showNewIssueDialog)) { showNewIssueDialog.set(false); return; }
			if (get(showAddRepo)) { showAddRepo.set(false); return; }
			if (get(removeRepoId) !== null) { removeRepoId.set(null); return; }
			return;
		}

		// Don't handle plain keys when in an input
		if (inInput) return;

		// Ctrl+1-9 — switch repo
		if (e.ctrlKey && !e.metaKey && e.key >= '1' && e.key <= '9') {
			e.preventDefault();
			const index = parseInt(e.key) - 1;
			const repoList = get(repos);
			if (index < repoList.length) {
				selectRepo(repoList[index].id);
				const repo = repoList[index];
				refreshIssues(repo.owner, repo.name);
			}
			return;
		}

		// 1-6 — scroll to column
		if (!e.metaKey && !e.ctrlKey && !e.altKey && e.key >= '1' && e.key <= '6') {
			e.preventDefault();
			const index = parseInt(e.key) - 1;
			if (index < COLUMN_ORDER.length) {
				const colId = COLUMN_ORDER[index];
				const el = document.querySelector(`[data-column-id="${colId}"]`);
				el?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' });
			}
			return;
		}
	}

	onMount(async () => {
		try {
			await initBackend();
			await checkAuth();
			const repoList = await loadRepos();
			const currentId = get(selectedRepoId);
			const repo = repoList.find((r) => r.id === currentId) ?? repoList[0];
			if (repo) {
				await refreshIssues(repo.owner, repo.name);
			}
			loading = false;
		} catch (e) {
			error = String(e);
			loading = false;
		}
	});
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="h-screen flex flex-col bg-background text-foreground dark">
	{#if loading}
		<div class="flex-1 flex items-center justify-center">
			<div class="flex items-center justify-center gap-2 text-sm text-muted-foreground">
				<Loader2 class="h-4 w-4 animate-spin" />
				Loading...
			</div>
		</div>
	{:else if error}
		<div class="flex-1 flex items-center justify-center">
			<div class="max-w-sm w-full p-6">
				<div class="text-sm text-red-400 bg-red-950/50 rounded-lg p-4 space-y-2">
					<p class="font-medium">Failed to initialize</p>
					<p class="text-xs font-mono">{error}</p>
				</div>
			</div>
		</div>
	{:else if !$authenticated}
		<!-- Login Screen -->
		<div class="flex-1 flex items-center justify-center">
			<div class="max-w-sm w-full space-y-6 p-6">
				<div class="text-center space-y-2">
					<h1 class="text-2xl font-bold text-foreground">AutoDev</h1>
					<p class="text-sm text-muted-foreground">Connect your GitHub account to get started</p>
				</div>

				{#if $authError}
					<div class="space-y-4">
						<div class="text-sm text-red-400 bg-red-950/50 rounded-lg p-4 space-y-2">
							<p class="font-medium">Sign in failed</p>
							<p class="text-xs font-mono whitespace-pre-wrap">{$authError}</p>
						</div>
						<button
							class="w-full px-4 py-2.5 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors"
							onclick={startAuth}
						>
							Try again
						</button>
					</div>
				{:else}
					<button
						class="w-full px-4 py-2.5 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
						onclick={startAuth}
						disabled={$authLoading}
					>
						{#if $authLoading}
							<span class="flex items-center justify-center gap-2">
								<Loader2 class="h-4 w-4 animate-spin" />
								Signing in...
							</span>
						{:else}
							Sign in with GitHub
						{/if}
					</button>
					<p class="text-xs text-muted-foreground text-center">
						Requires <code class="text-foreground/70">gh</code> CLI to be installed and authenticated
					</p>
				{/if}
			</div>
		</div>
	{:else}
		<!-- Main App -->
		<!-- Top Bar -->
		<header class="shrink-0 flex items-center justify-between px-4 py-2 border-b border-border bg-background/80 backdrop-blur-sm" data-tauri-drag-region>
			<div class="flex items-center gap-3">
				<span class="text-sm font-semibold text-foreground">AutoDev</span>
				<RepoSelector />
			</div>
			<div class="flex items-center gap-2">
				{#if $currentUser}
					<img
						src={$currentUser.avatar_url}
						alt={$currentUser.login}
						class="h-6 w-6 rounded-full ring-1 ring-border"
					/>
				{/if}
				<button
					class="p-1.5 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
					onclick={() => showSettings.set(true)}
					title="Settings"
				>
					<Settings class="h-4 w-4" />
				</button>
			</div>
		</header>

		<!-- Kanban Board -->
		<main class="flex-1 min-h-0 flex flex-col">
			<KanbanBoard />
		</main>

		<!-- Bottom Bar -->
		<footer class="shrink-0 flex items-center justify-between px-4 py-2 border-t border-border bg-background/80">
			<button
				class="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors"
				onclick={() => showNewIssueDialog.set(true)}
			>
				<Plus class="h-3.5 w-3.5" />
				New Issue
			</button>
			<div class="text-xs text-muted-foreground">
				{#if $currentUser}
					{$currentUser.login}
				{/if}
			</div>
		</footer>

		<!-- Overlays -->
		<CardDetail />
		<NewIssueDialog />
		<SettingsDialog />
		<AddRepoDialog />
		<RemoveRepoDialog />
		<CommandPalette />
		<KeyboardShortcuts />
	{/if}
</div>
