<script lang="ts">
	import { Dialog } from 'bits-ui';
	import { showNewIssueDialog } from '$lib/stores/ui';
	import { repos, selectedRepoId } from '$lib/stores/repos';
	import { currentUser } from '$lib/stores/auth';
	import * as backend from '$lib/stores/backend';
	import { X } from 'lucide-svelte';

	let title = $state('');
	let body = $state('');
	let assignee = $state('');

	let selectedRepo = $derived($repos.find((r) => r.id === $selectedRepoId));

	// Set default assignee from current user
	$effect(() => {
		if ($showNewIssueDialog && $currentUser) {
			assignee = $currentUser.login;
		}
	});

	function resetForm() {
		title = '';
		body = '';
		assignee = $currentUser?.login ?? '';
	}

	function handleClose() {
		showNewIssueDialog.set(false);
		resetForm();
	}

	async function handleCreate() {
		if (!title.trim() || !selectedRepo) return;
		await backend.createIssue(
			selectedRepo.owner,
			selectedRepo.name,
			title.trim(),
			body.trim(),
			null
		);
		handleClose();
	}

	async function handleCreateAndStart() {
		if (!title.trim() || !selectedRepo) return;
		await backend.createIssue(
			selectedRepo.owner,
			selectedRepo.name,
			title.trim(),
			body.trim(),
			assignee.trim() || null
		);
		handleClose();
	}
</script>

<Dialog.Root
	open={$showNewIssueDialog}
	onOpenChange={(open) => { if (!open) handleClose(); }}
>
	<Dialog.Portal>
		<Dialog.Overlay class="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm" />
		<Dialog.Content
			class="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 bg-popover border border-border rounded-lg shadow-xl"
		>
			<div class="flex items-center justify-between p-4 border-b border-border">
				<Dialog.Title class="text-base font-semibold text-foreground">New Issue</Dialog.Title>
				<Dialog.Close
					class="p-1 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
				>
					<X class="h-4 w-4" />
				</Dialog.Close>
			</div>

			<div class="p-4 space-y-4">
				{#if !selectedRepo}
					<div class="text-sm text-muted-foreground bg-muted rounded-lg p-3">
						Please select a repository first.
					</div>
				{:else}
					<div class="text-xs text-muted-foreground">
						Creating issue in <span class="font-medium text-foreground">{selectedRepo.full_name}</span>
					</div>

					<div class="space-y-1.5">
						<label for="issue-title" class="text-sm font-medium text-foreground">Title</label>
						<input
							id="issue-title"
							class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
							placeholder="Issue title"
							bind:value={title}
						/>
					</div>

					<div class="space-y-1.5">
						<label for="issue-body" class="text-sm font-medium text-foreground">Description</label>
						<textarea
							id="issue-body"
							class="w-full h-32 bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-none"
							placeholder="Describe the issue..."
							bind:value={body}
						></textarea>
					</div>

					<div class="space-y-1.5">
						<label for="issue-assignee" class="text-sm font-medium text-foreground">Assignee</label>
						<input
							id="issue-assignee"
							class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
							placeholder="GitHub username"
							bind:value={assignee}
						/>
					</div>
				{/if}
			</div>

			<div class="flex justify-end gap-2 p-4 border-t border-border">
				<button
					class="px-3 py-2 text-sm rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
					onclick={handleClose}
				>
					Cancel
				</button>
				<button
					class="px-3 py-2 text-sm rounded-md bg-secondary text-secondary-foreground hover:bg-secondary/80 transition-colors disabled:opacity-50"
					onclick={handleCreate}
					disabled={!title.trim() || !selectedRepo}
				>
					Create
				</button>
				<button
					class="px-3 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
					onclick={handleCreateAndStart}
					disabled={!title.trim() || !selectedRepo}
				>
					Create & Start
				</button>
			</div>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
