<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { repos, selectedRepoId, selectRepo as persistSelectRepo } from '$lib/stores/repos';
	import { refreshIssues } from '$lib/stores/issues';
	import { showAddRepo, removeRepoId } from '$lib/stores/ui';
	import { ChevronDown, Plus, FolderGit2, Trash2 } from 'lucide-svelte';

	let open = $state(false);

	let selectedRepo = $derived($repos.find((r) => r.id === $selectedRepoId));

	function selectRepo(id: number) {
		persistSelectRepo(id);
		open = false;
		const repo = $repos.find((r) => r.id === id);
		if (repo) {
			refreshIssues(repo.owner, repo.name);
		}
	}

	function handleRemove(e: MouseEvent, repoId: number) {
		e.stopPropagation();
		open = false;
		removeRepoId.set(repoId);
	}
</script>

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger
		class="flex items-center gap-2 px-3 py-1.5 rounded-md hover:bg-muted text-sm font-medium text-foreground transition-colors"
	>
		<FolderGit2 class="h-4 w-4 text-muted-foreground" />
		<span class="max-w-[200px] truncate">
			{selectedRepo ? selectedRepo.full_name : 'Select Repository'}
		</span>
		<ChevronDown class="h-3.5 w-3.5 text-muted-foreground" />
	</DropdownMenu.Trigger>

	<DropdownMenu.Portal>
		<DropdownMenu.Content
			class="z-50 min-w-[200px] bg-popover border border-border rounded-lg shadow-lg p-1 animate-in fade-in-0 zoom-in-95"
			sideOffset={4}
		>
			{#if $repos.length === 0}
				<div class="px-3 py-2 text-sm text-muted-foreground">No repositories</div>
			{/if}

			{#each $repos as repo (repo.id)}
				<DropdownMenu.Item
					class="group flex items-center gap-2 px-3 py-2 text-sm rounded-md cursor-pointer hover:bg-accent hover:text-accent-foreground outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground {repo.id === $selectedRepoId ? 'bg-accent/50' : ''}"
					onSelect={() => selectRepo(repo.id)}
				>
					<FolderGit2 class="h-3.5 w-3.5 text-muted-foreground" />
					<span class="flex-1 truncate">{repo.full_name}</span>
					<button
						class="hidden group-hover:flex items-center justify-center h-5 w-5 rounded hover:bg-red-500/20 hover:text-red-400 text-muted-foreground/50 transition-colors"
						onclick={(e) => handleRemove(e, repo.id)}
					>
						<Trash2 class="h-3 w-3" />
					</button>
				</DropdownMenu.Item>
			{/each}

			<DropdownMenu.Separator class="my-1 h-px bg-border" />

			<DropdownMenu.Item
				class="flex items-center gap-2 px-3 py-2 text-sm rounded-md cursor-pointer hover:bg-accent hover:text-accent-foreground outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
				onSelect={() => { showAddRepo.set(true); }}
			>
				<Plus class="h-3.5 w-3.5 text-muted-foreground" />
				Add Repository
			</DropdownMenu.Item>
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>
