<script lang="ts">
	import { showAddRepo } from '$lib/stores/ui';
	import { repos, selectRepo } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import type { GitHubRepo } from '$lib/types';
	import { FolderGit2, Loader2, Search } from 'lucide-svelte';

	let filter = $state('');
	let ghRepos = $state<GitHubRepo[]>([]);
	let loading = $state(false);
	let error = $state('');
	let adding = $state<string | null>(null);

	let addedSet = $derived(new Set($repos.map((r) => r.full_name)));

	let filtered = $derived(
		filter.trim()
			? ghRepos.filter((r) => r.full_name.toLowerCase().includes(filter.trim().toLowerCase()))
			: ghRepos
	);

	$effect(() => {
		if ($showAddRepo) {
			filter = '';
			error = '';
			loading = true;
			backend.listUserRepos()
				.then((list) => { ghRepos = list; })
				.catch((e) => { error = e instanceof Error ? e.message : String(e); })
				.finally(() => { loading = false; });
		}
	});

	async function handleSelect(repo: GitHubRepo) {
		if (addedSet.has(repo.full_name) || adding) return;
		adding = repo.full_name;
		error = '';
		try {
			const added = await backend.addRepo(repo.owner.login, repo.name);
			repos.update((list) => [...list, added]);
			selectRepo(added.id);
			close();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			adding = null;
		}
	}

	function close() {
		showAddRepo.set(false);
		filter = '';
		ghRepos = [];
		error = '';
	}
</script>

{#if $showAddRepo}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 z-50 flex items-center justify-center" onkeydown={(e) => { if (e.key === 'Escape') close(); }}>
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="absolute inset-0 bg-black/40" onclick={close}></div>
		<div class="relative bg-popover border border-border rounded-lg shadow-xl w-96 flex flex-col max-h-[70vh] overflow-hidden">
			<div class="shrink-0 p-4 pb-3 border-b border-border">
				<h3 class="text-sm font-semibold text-foreground mb-3">Add Repository</h3>
				<div class="relative">
					<Search class="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
					<input
						class="w-full bg-muted rounded-md pl-9 pr-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
						placeholder="Filter repositories..."
						bind:value={filter}
					/>
				</div>
			</div>

			<div class="flex-1 min-h-0 overflow-y-auto p-1">
				{#if loading}
					<div class="flex items-center justify-center py-8 text-muted-foreground">
						<Loader2 class="h-4 w-4 animate-spin mr-2" />
						<span class="text-sm">Loading repositories...</span>
					</div>
				{:else if error}
					<div class="px-3 py-4 text-sm text-red-400">{error}</div>
				{:else if filtered.length === 0}
					<div class="px-3 py-4 text-sm text-muted-foreground">
						{filter.trim() ? 'No matching repositories' : 'No repositories found'}
					</div>
				{:else}
					{#each filtered as repo (repo.id)}
						{@const alreadyAdded = addedSet.has(repo.full_name)}
						{@const isAdding = adding === repo.full_name}
						<button
							class="w-full flex items-center gap-2 px-3 py-2 text-sm rounded-md text-left transition-colors
								{alreadyAdded ? 'text-muted-foreground/50 cursor-default' : 'hover:bg-accent hover:text-accent-foreground cursor-pointer text-foreground'}"
							onclick={() => handleSelect(repo)}
							disabled={alreadyAdded || !!adding}
						>
							<FolderGit2 class="h-3.5 w-3.5 shrink-0 {alreadyAdded ? 'text-muted-foreground/30' : 'text-muted-foreground'}" />
							<span class="truncate">{repo.full_name}</span>
							{#if alreadyAdded}
								<span class="ml-auto text-xs text-muted-foreground/50 shrink-0">Added</span>
							{/if}
							{#if isAdding}
								<Loader2 class="ml-auto h-3.5 w-3.5 animate-spin shrink-0" />
							{/if}
						</button>
					{/each}
				{/if}
			</div>

			<div class="shrink-0 p-3 border-t border-border flex justify-end">
				<button
					class="px-3 py-1.5 text-sm rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
					onclick={close}
				>
					Cancel
				</button>
			</div>
		</div>
	</div>
{/if}
