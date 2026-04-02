<script lang="ts">
	import { Dialog } from 'bits-ui';
	import { showNewIssueDialog } from '$lib/stores/ui';
	import { repos, selectedRepoId } from '$lib/stores/repos';
	import { currentUser } from '$lib/stores/auth';
	import * as backend from '$lib/stores/backend';
	import type { GitHubUser } from '$lib/types';
	import { X, ChevronDown, Check, Loader2 } from 'lucide-svelte';

	let title = $state('');
	let body = $state('');
	let selectedAssignee = $state<GitHubUser | null>(null);
	let collaborators = $state<GitHubUser[]>([]);
	let loadingCollaborators = $state(false);
	let dropdownOpen = $state(false);

	let selectedRepo = $derived($repos.find((r) => r.id === $selectedRepoId));

	let isCurrentUser = $derived(
		selectedAssignee !== null &&
			$currentUser !== null &&
			selectedAssignee.login === $currentUser.login
	);

	let createAndStartDisabled = $derived(
		!title.trim() || !selectedRepo || !isCurrentUser
	);

	// Fetch collaborators and set default assignee when dialog opens
	$effect(() => {
		if ($showNewIssueDialog && selectedRepo) {
			loadingCollaborators = true;
			backend
				.listCollaborators(selectedRepo.owner, selectedRepo.name)
				.then((users) => {
					collaborators = users;
					// Default to current user if they are in the list
					if ($currentUser) {
						const match = users.find((u) => u.login === $currentUser!.login);
						selectedAssignee = match ?? $currentUser;
					}
				})
				.catch(() => {
					collaborators = [];
				})
				.finally(() => {
					loadingCollaborators = false;
				});
		}
	});

	function resetForm() {
		title = '';
		body = '';
		selectedAssignee = $currentUser ?? null;
		collaborators = [];
		dropdownOpen = false;
	}

	function handleClose() {
		showNewIssueDialog.set(false);
		resetForm();
	}

	function selectAssignee(user: GitHubUser | null) {
		selectedAssignee = user;
		dropdownOpen = false;
	}

	function handleDropdownToggle() {
		dropdownOpen = !dropdownOpen;
	}

	function handleClickOutside(event: MouseEvent) {
		const target = event.target as HTMLElement;
		if (!target.closest('.assignee-dropdown')) {
			dropdownOpen = false;
		}
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
		if (!title.trim() || !selectedRepo || !selectedAssignee) return;
		await backend.createIssue(
			selectedRepo.owner,
			selectedRepo.name,
			title.trim(),
			body.trim(),
			selectedAssignee.login
		);
		handleClose();
	}
</script>

<svelte:window onclick={handleClickOutside} />

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
						<span id="assignee-label" class="text-sm font-medium text-foreground">Assignee</span>
						<div class="assignee-dropdown relative">
							<button
								type="button"
								aria-labelledby="assignee-label"
								class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring flex items-center justify-between gap-2"
								onclick={handleDropdownToggle}
							>
								{#if loadingCollaborators}
									<span class="flex items-center gap-2 text-muted-foreground">
										<Loader2 class="h-4 w-4 animate-spin" />
										Loading...
									</span>
								{:else if selectedAssignee}
									<span class="flex items-center gap-2">
										<img
											src={selectedAssignee.avatar_url}
											alt={selectedAssignee.login}
											class="h-5 w-5 rounded-full"
										/>
										{selectedAssignee.login}
									</span>
								{:else}
									<span class="text-muted-foreground">Unassigned</span>
								{/if}
								<ChevronDown class="h-4 w-4 text-muted-foreground shrink-0" />
							</button>

							{#if dropdownOpen}
								<div
									class="absolute left-0 right-0 top-full mt-1 z-10 bg-popover border border-border rounded-md shadow-lg max-h-48 overflow-y-auto"
								>
									<button
										type="button"
										class="w-full px-3 py-2 text-sm text-left flex items-center gap-2 hover:bg-muted transition-colors"
										onclick={() => selectAssignee(null)}
									>
										<span class="h-5 w-5 flex items-center justify-center">
											{#if selectedAssignee === null}
												<Check class="h-3.5 w-3.5 text-foreground" />
											{/if}
										</span>
										<span class="text-muted-foreground">Unassigned</span>
									</button>

									{#each collaborators as collaborator (collaborator.id)}
										<button
											type="button"
											class="w-full px-3 py-2 text-sm text-left flex items-center gap-2 hover:bg-muted transition-colors"
											onclick={() => selectAssignee(collaborator)}
										>
											<span class="h-5 w-5 flex items-center justify-center">
												{#if selectedAssignee?.login === collaborator.login}
													<Check class="h-3.5 w-3.5 text-foreground" />
												{/if}
											</span>
											<img
												src={collaborator.avatar_url}
												alt={collaborator.login}
												class="h-5 w-5 rounded-full"
											/>
											<span class="text-foreground">{collaborator.login}</span>
										</button>
									{/each}
								</div>
							{/if}
						</div>
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
					disabled={createAndStartDisabled}
				>
					Create & Start
				</button>
			</div>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
