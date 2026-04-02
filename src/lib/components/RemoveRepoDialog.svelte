<script lang="ts">
	import { removeRepoId } from '$lib/stores/ui';
	import { removeRepo } from '$lib/stores/repos';
	import { getRepoRemovalInfo } from '$lib/stores/backend';
	import { Loader2, Trash2, AlertTriangle } from 'lucide-svelte';
	import type { RepoRemovalInfo } from '$lib/types';

	let loading = $state(false);
	let removing = $state(false);
	let error = $state('');
	let info = $state<RepoRemovalInfo | null>(null);

	$effect(() => {
		if ($removeRepoId != null) {
			loading = true;
			error = '';
			info = null;
			getRepoRemovalInfo($removeRepoId)
				.then((result) => { info = result; })
				.catch((e) => { error = e instanceof Error ? e.message : String(e); })
				.finally(() => { loading = false; });
		}
	});

	async function handleRemove() {
		if ($removeRepoId == null || removing) return;
		removing = true;
		error = '';
		try {
			await removeRepo($removeRepoId);
			close();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			removing = false;
		}
	}

	function close() {
		removeRepoId.set(null);
		info = null;
		error = '';
		removing = false;
	}
</script>

{#if $removeRepoId != null}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 z-50 flex items-center justify-center" onkeydown={(e) => { if (e.key === 'Escape') close(); }}>
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="absolute inset-0 bg-black/40" onclick={close}></div>
		<div class="relative bg-popover border border-border rounded-lg shadow-xl w-96 flex flex-col max-h-[70vh] overflow-hidden">
			{#if loading}
				<div class="flex items-center justify-center py-12 text-muted-foreground">
					<Loader2 class="h-4 w-4 animate-spin mr-2" />
					<span class="text-sm">Loading removal info...</span>
				</div>
			{:else if error && !info}
				<div class="p-4">
					<div class="text-sm text-red-400">{error}</div>
					<div class="mt-3 flex justify-end">
						<button
							class="px-3 py-1.5 text-sm rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
							onclick={close}
						>
							Close
						</button>
					</div>
				</div>
			{:else if info}
				<div class="p-4">
					<div class="flex items-center gap-2 mb-3">
						<AlertTriangle class="h-5 w-5 text-red-400 shrink-0" />
						<h3 class="text-sm font-semibold text-foreground">Remove Repository</h3>
					</div>

					<p class="text-sm text-foreground mb-3">
						Are you sure you want to remove <span class="font-semibold">{info.repo_name}</span>?
					</p>

					{#if error}
						<div class="text-sm text-red-400 mb-3">{error}</div>
					{/if}

					<p class="text-sm text-muted-foreground mb-2">The following will be permanently deleted:</p>

					<div class="bg-muted/50 rounded-md border border-border p-3 mb-4">
						{#if info.local_path}
							<div class="text-sm text-muted-foreground">
								<span>&#8226;</span> Local repo path setting: <span class="font-mono">{info.local_path}</span>
							</div>
						{/if}
						{#each info.worktree_paths as path (path)}
							<div class="text-sm text-muted-foreground">
								<span>&#8226;</span> Worktree: <span class="font-mono">{path}</span>
							</div>
						{/each}
						{#if info.session_count > 0}
							<div class="text-sm text-muted-foreground">
								<span>&#8226;</span> {info.session_count} session record{info.session_count === 1 ? '' : 's'}
							</div>
						{/if}
						{#if info.log_count > 0}
							<div class="text-sm text-muted-foreground">
								<span>&#8226;</span> {info.log_count} session log{info.log_count === 1 ? '' : 's'}
							</div>
						{/if}
						<div class="text-sm text-muted-foreground">
							<span>&#8226;</span> Database entry for <span class="font-semibold">{info.repo_name}</span>
						</div>
					</div>
				</div>

				<div class="shrink-0 p-3 border-t border-border flex justify-end gap-2">
					<button
						class="px-3 py-1.5 text-sm rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
						onclick={close}
						disabled={removing}
					>
						Cancel
					</button>
					<button
						class="px-3 py-1.5 text-sm rounded-md bg-red-600 hover:bg-red-700 text-white transition-colors flex items-center gap-1.5 disabled:opacity-50"
						onclick={handleRemove}
						disabled={removing}
					>
						{#if removing}
							<Loader2 class="h-3.5 w-3.5 animate-spin" />
						{:else}
							<Trash2 class="h-3.5 w-3.5" />
						{/if}
						Remove
					</button>
				</div>
			{/if}
		</div>
	</div>
{/if}
