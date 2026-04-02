<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { repos, selectedRepoId, selectRepo as persistSelectRepo } from '$lib/stores/repos';
	import { refreshIssues } from '$lib/stores/issues';
	import { showAddRepo } from '$lib/stores/ui';
	import { ChevronDown, Plus, FolderGit2 } from 'lucide-svelte';

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
					class="flex items-center gap-2 px-3 py-2 text-sm rounded-md cursor-pointer hover:bg-accent hover:text-accent-foreground outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground {repo.id === $selectedRepoId ? 'bg-accent/50' : ''}"
					onSelect={() => selectRepo(repo.id)}
				>
					<FolderGit2 class="h-3.5 w-3.5 text-muted-foreground" />
					{repo.full_name}
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
