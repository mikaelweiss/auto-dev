<script lang="ts">
	import { showCommandPalette, selectedIssue, showNewIssueDialog, showSettings } from '$lib/stores/ui';
	import { issues } from '$lib/stores/issues';
	import { repos, selectedRepoId, selectRepo } from '$lib/stores/repos';
	import { refreshIssues } from '$lib/stores/issues';
	import { COLUMN_ORDER, COLUMN_CONFIG } from '$lib/types';
	import { get } from 'svelte/store';
	import { Search, Plus, Settings, RefreshCw, FolderGit2, CircleDot, Columns3 } from 'lucide-svelte';

	let query = $state('');
	let selectedIndex = $state(0);
	let inputEl = $state<HTMLInputElement | null>(null);

	$effect(() => {
		if ($showCommandPalette) {
			query = '';
			selectedIndex = 0;
			requestAnimationFrame(() => inputEl?.focus());
		}
	});

	interface Command {
		id: string;
		label: string;
		hint?: string;
		iconType: 'plus' | 'settings' | 'refresh' | 'repo' | 'issue' | 'column';
		action: () => void;
		category: string;
	}

	let commands = $derived.by(() => {
		const cmds: Command[] = [];

		// Actions
		cmds.push({
			id: 'new-issue',
			label: 'New Issue',
			hint: '⌘N',
			iconType: 'plus',
			action: () => showNewIssueDialog.set(true),
			category: 'Actions'
		});
		cmds.push({
			id: 'settings',
			label: 'Settings',
			hint: '⌘,',
			iconType: 'settings',
			action: () => showSettings.set(true),
			category: 'Actions'
		});
		cmds.push({
			id: 'refresh',
			label: 'Refresh Issues',
			iconType: 'refresh',
			action: () => {
				const repo = $repos.find((r) => r.id === $selectedRepoId);
				if (repo) refreshIssues(repo.owner, repo.name);
			},
			category: 'Actions'
		});

		// Columns
		for (let i = 0; i < COLUMN_ORDER.length; i++) {
			const colId = COLUMN_ORDER[i];
			cmds.push({
				id: `column-${colId}`,
				label: `Jump to ${COLUMN_CONFIG[colId].label}`,
				hint: `${i + 1}`,
				iconType: 'column',
				action: () => {
					const el = document.querySelector(`[data-column-id="${colId}"]`);
					el?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' });
				},
				category: 'Columns'
			});
		}

		// Repos
		for (let i = 0; i < $repos.length; i++) {
			const repo = $repos[i];
			cmds.push({
				id: `repo-${repo.id}`,
				label: repo.full_name,
				hint: i < 9 ? `⌃${i + 1}` : undefined,
				iconType: 'repo',
				action: () => {
					selectRepo(repo.id);
					refreshIssues(repo.owner, repo.name);
				},
				category: 'Repositories'
			});
		}

		// Issues for current repo
		const currentRepo = $repos.find((r) => r.id === $selectedRepoId);
		if (currentRepo) {
			const repoIssues = $issues.filter(
				(i) =>
					i.repo_owner === currentRepo.owner &&
					i.repo_name === currentRepo.name &&
					i.state === 'open'
			);
			for (const issue of repoIssues) {
				cmds.push({
					id: `issue-${issue.id}`,
					label: `#${issue.number} ${issue.title}`,
					iconType: 'issue',
					action: () => selectedIssue.set(issue),
					category: 'Issues'
				});
			}
		}

		return cmds;
	});

	let filtered = $derived.by(() => {
		if (!query.trim()) return commands;
		const q = query.toLowerCase();
		return commands.filter((c) => c.label.toLowerCase().includes(q));
	});

	// Reset selection when the filtered list changes
	let prevFilteredRef: Command[] = [];
	$effect(() => {
		if (filtered !== prevFilteredRef) {
			prevFilteredRef = filtered;
			selectedIndex = 0;
		}
	});

	// Scroll selected item into view
	$effect(() => {
		if ($showCommandPalette && filtered.length > 0) {
			const el = document.getElementById(`cmd-item-${selectedIndex}`);
			el?.scrollIntoView({ block: 'nearest' });
		}
	});

	// Group filtered commands by category
	let groupedCommands = $derived.by(() => {
		const groups: { category: string; commands: Command[] }[] = [];
		const seen = new Set<string>();
		for (const cmd of filtered) {
			if (!seen.has(cmd.category)) {
				seen.add(cmd.category);
				groups.push({ category: cmd.category, commands: [] });
			}
			groups.find((g) => g.category === cmd.category)!.commands.push(cmd);
		}
		return groups;
	});

	function getFlatIndex(groupIdx: number, cmdIdx: number): number {
		let flat = 0;
		for (let g = 0; g < groupIdx; g++) {
			flat += groupedCommands[g].commands.length;
		}
		return flat + cmdIdx;
	}

	function close() {
		showCommandPalette.set(false);
	}

	function execute(cmd: Command) {
		close();
		cmd.action();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = Math.max(selectedIndex - 1, 0);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			if (filtered[selectedIndex]) {
				execute(filtered[selectedIndex]);
			}
		} else if (e.key === 'Escape') {
			e.preventDefault();
			close();
		}
	}

	function handleOverlayClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			close();
		}
	}
</script>

{#if $showCommandPalette}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-[60] flex items-start justify-center pt-[15vh]"
		onclick={handleOverlayClick}
		onkeydown={handleKeydown}
	>
		<div class="absolute inset-0 bg-black/40 backdrop-blur-sm"></div>

		<div
			class="relative w-full max-w-lg bg-popover border border-border rounded-xl shadow-2xl overflow-hidden animate-palette-in"
		>
			<!-- Search input -->
			<div class="flex items-center gap-3 px-4 py-3 border-b border-border">
				<Search class="h-4 w-4 text-muted-foreground shrink-0" />
				<input
					bind:this={inputEl}
					bind:value={query}
					class="flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
					placeholder="Search commands, repos, issues..."
				/>
				<kbd
					class="hidden sm:inline-flex items-center px-1.5 py-0.5 rounded bg-muted text-[10px] font-mono text-muted-foreground"
					>ESC</kbd
				>
			</div>

			<!-- Results -->
			<div class="max-h-[50vh] overflow-y-auto py-1">
				{#if filtered.length === 0}
					<div class="px-4 py-8 text-center text-sm text-muted-foreground">No results found</div>
				{:else}
					{#each groupedCommands as group, groupIdx}
						<div class="px-3 pt-2 pb-1">
							<span class="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
								{group.category}
							</span>
						</div>
						{#each group.commands as cmd, cmdIdx}
							{@const flatIdx = getFlatIndex(groupIdx, cmdIdx)}
							<button
								id="cmd-item-{flatIdx}"
								class="w-full flex items-center gap-3 px-4 py-2 text-sm text-left transition-colors
									{flatIdx === selectedIndex
									? 'bg-accent text-accent-foreground'
									: 'text-foreground hover:bg-muted'}"
								onclick={() => execute(cmd)}
								onmouseenter={() => {
									selectedIndex = flatIdx;
								}}
							>
								{#if cmd.iconType === 'plus'}
									<Plus class="h-4 w-4 text-muted-foreground shrink-0" />
								{:else if cmd.iconType === 'settings'}
									<Settings class="h-4 w-4 text-muted-foreground shrink-0" />
								{:else if cmd.iconType === 'refresh'}
									<RefreshCw class="h-4 w-4 text-muted-foreground shrink-0" />
								{:else if cmd.iconType === 'repo'}
									<FolderGit2 class="h-4 w-4 text-muted-foreground shrink-0" />
								{:else if cmd.iconType === 'issue'}
									<CircleDot class="h-4 w-4 text-muted-foreground shrink-0" />
								{:else if cmd.iconType === 'column'}
									<Columns3 class="h-4 w-4 text-muted-foreground shrink-0" />
								{/if}
								<span class="flex-1 truncate">{cmd.label}</span>
								{#if cmd.hint}
									<kbd
										class="text-[11px] font-mono text-muted-foreground bg-muted px-1.5 py-0.5 rounded shrink-0"
										>{cmd.hint}</kbd
									>
								{/if}
							</button>
						{/each}
					{/each}
				{/if}
			</div>
		</div>
	</div>
{/if}

<style>
	.animate-palette-in {
		animation: paletteIn 0.15s ease-out;
	}
	@keyframes paletteIn {
		from {
			opacity: 0;
			transform: scale(0.97) translateY(-4px);
		}
		to {
			opacity: 1;
			transform: scale(1) translateY(0);
		}
	}
</style>
