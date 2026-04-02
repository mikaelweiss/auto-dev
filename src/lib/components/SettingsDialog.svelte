<script lang="ts">
	import { Dialog, Tabs, Switch } from 'bits-ui';
	import { showSettings } from '$lib/stores/ui';
	import { appSettings, agentPrompts, loadSettings, loadPrompts } from '$lib/stores/settings';
	import { repos, selectedRepoId } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import type { SessionStage, AppSettings } from '$lib/types';
	import { X } from 'lucide-svelte';

	// Local copies of settings for editing
	let localSettings = $state<AppSettings>({
		sleep_prevention: true,
		notifications_enabled: true,
		poll_interval_seconds: 15
	});

	// Local copies of prompts
	let localPrompts = $state<Record<SessionStage, string>>({
		spec: '',
		implement: '',
		review: '',
		ci_fix: '',
		merge_conflict: ''
	});

	// Repo-specific settings
	let repoLocalPath = $state('');
	let repoSetupScript = $state('');
	let repoRunScript = $state('');
	let repoBaseBranch = $state('');
	let repoBranchPrefix = $state('');
	let repoWorktreeDir = $state('');

	let selectedRepo = $derived($repos.find((r) => r.id === $selectedRepoId));

	const STAGE_LABELS: Record<SessionStage, string> = {
		spec: 'Spec',
		implement: 'Implement',
		review: 'Review',
		ci_fix: 'CI Fix',
		merge_conflict: 'Merge Conflict'
	};

	const STAGES: SessionStage[] = ['spec', 'implement', 'review', 'ci_fix', 'merge_conflict'];

	// Sync local state when dialog opens
	$effect(() => {
		if ($showSettings) {
			localSettings = { ...$appSettings };

			for (const prompt of $agentPrompts) {
				localPrompts[prompt.stage] = prompt.prompt_text;
			}

			if (selectedRepo) {
				repoSetupScript = selectedRepo.setup_script;
				repoRunScript = selectedRepo.run_script;
				repoBaseBranch = selectedRepo.base_branch;
				repoBranchPrefix = selectedRepo.branch_prefix;
				repoWorktreeDir = selectedRepo.worktree_dir;

				// Load repo local path from settings
				backend.getRepoPath(selectedRepo.id).then((path) => {
					repoLocalPath = path ?? '';
				});
			}

			// Request fresh settings from backend
			loadSettings();
			loadPrompts();
		}
	});

	function handleClose() {
		showSettings.set(false);
	}

	function saveAppSettings() {
		backend.updateSettings(localSettings);
	}

	function savePrompt(stage: SessionStage) {
		backend.updatePrompt(stage, localPrompts[stage]);
	}

	function saveAllPrompts() {
		for (const stage of STAGES) {
			if (localPrompts[stage]) {
				savePrompt(stage);
			}
		}
	}
</script>

<Dialog.Root
	open={$showSettings}
	onOpenChange={(open) => { if (!open) handleClose(); }}
>
	<Dialog.Portal>
		<Dialog.Overlay class="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm" />
		<Dialog.Content
			class="fixed left-1/2 top-1/2 z-50 w-full max-w-2xl max-h-[85vh] -translate-x-1/2 -translate-y-1/2 bg-popover border border-border rounded-lg shadow-xl flex flex-col"
		>
			<div class="flex items-center justify-between p-4 border-b border-border shrink-0">
				<Dialog.Title class="text-base font-semibold text-foreground">Settings</Dialog.Title>
				<Dialog.Close
					class="p-1 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
				>
					<X class="h-4 w-4" />
				</Dialog.Close>
			</div>

			<Tabs.Root value="app" class="flex-1 flex flex-col min-h-0">
				<Tabs.List class="flex border-b border-border px-4 shrink-0">
					<Tabs.Trigger
						value="app"
						class="px-3 py-2 text-sm text-muted-foreground border-b-2 border-transparent data-[state=active]:text-foreground data-[state=active]:border-primary transition-colors"
					>
						App Settings
					</Tabs.Trigger>
					<Tabs.Trigger
						value="prompts"
						class="px-3 py-2 text-sm text-muted-foreground border-b-2 border-transparent data-[state=active]:text-foreground data-[state=active]:border-primary transition-colors"
					>
						Agent Prompts
					</Tabs.Trigger>
					<Tabs.Trigger
						value="repo"
						class="px-3 py-2 text-sm text-muted-foreground border-b-2 border-transparent data-[state=active]:text-foreground data-[state=active]:border-primary transition-colors"
					>
						Repository
					</Tabs.Trigger>
				</Tabs.List>

				<!-- App Settings Tab -->
				<Tabs.Content value="app" class="flex-1 overflow-y-auto p-4 space-y-5">
					<div class="space-y-4">
						<div class="flex items-center justify-between">
							<div>
								<p class="text-sm font-medium text-foreground">Sleep Prevention</p>
								<p class="text-xs text-muted-foreground">Prevent the system from sleeping while agents run</p>
							</div>
							<Switch.Root
								checked={localSettings.sleep_prevention}
								onCheckedChange={(checked) => { localSettings.sleep_prevention = checked; }}
								class="relative h-5 w-9 rounded-full bg-muted transition-colors data-[state=checked]:bg-primary"
							>
								<Switch.Thumb class="block h-4 w-4 rounded-full bg-background shadow-sm transition-transform data-[state=checked]:translate-x-4 translate-x-0.5" />
							</Switch.Root>
						</div>

						<div class="flex items-center justify-between">
							<div>
								<p class="text-sm font-medium text-foreground">Notifications</p>
								<p class="text-xs text-muted-foreground">Show system notifications for important events</p>
							</div>
							<Switch.Root
								checked={localSettings.notifications_enabled}
								onCheckedChange={(checked) => { localSettings.notifications_enabled = checked; }}
								class="relative h-5 w-9 rounded-full bg-muted transition-colors data-[state=checked]:bg-primary"
							>
								<Switch.Thumb class="block h-4 w-4 rounded-full bg-background shadow-sm transition-transform data-[state=checked]:translate-x-4 translate-x-0.5" />
							</Switch.Root>
						</div>

						<div class="space-y-1.5">
							<label for="poll-interval" class="text-sm font-medium text-foreground">Poll Interval (seconds)</label>
							<p class="text-xs text-muted-foreground">How often to check GitHub for updates</p>
							<input
								id="poll-interval"
								type="number"
								min="5"
								max="300"
								class="w-24 bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
								bind:value={localSettings.poll_interval_seconds}
							/>
						</div>
					</div>

					<div class="flex justify-end pt-2">
						<button
							class="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
							onclick={saveAppSettings}
						>
							Save Settings
						</button>
					</div>
				</Tabs.Content>

				<!-- Agent Prompts Tab -->
				<Tabs.Content value="prompts" class="flex-1 overflow-y-auto p-4 space-y-4">
					{#each STAGES as stage (stage)}
						<div class="space-y-1.5">
							<label for="prompt-{stage}" class="text-sm font-medium text-foreground">{STAGE_LABELS[stage]}</label>
							<textarea
								id="prompt-{stage}"
								class="w-full h-28 bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-y font-mono"
								placeholder="Custom prompt for {STAGE_LABELS[stage]} stage..."
								bind:value={localPrompts[stage]}
							></textarea>
						</div>
					{/each}

					<div class="flex justify-end pt-2">
						<button
							class="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
							onclick={saveAllPrompts}
						>
							Save Prompts
						</button>
					</div>
				</Tabs.Content>

				<!-- Repository Tab -->
				<Tabs.Content value="repo" class="flex-1 overflow-y-auto p-4 space-y-4">
					{#if !selectedRepo}
						<div class="text-sm text-muted-foreground bg-muted rounded-lg p-4">
							Select a repository from the top bar to configure its settings.
						</div>
					{:else}
						<div class="text-xs text-muted-foreground">
							Settings for <span class="font-medium text-foreground">{selectedRepo.full_name}</span>
						</div>

						<div class="space-y-1.5">
							<label for="repo-local-path" class="text-sm font-medium text-foreground">Local Path</label>
							<p class="text-xs text-muted-foreground">Absolute path to the repo clone on disk (required for worktrees)</p>
							<input
								id="repo-local-path"
								class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring font-mono"
								placeholder="/Users/you/code/my-repo"
								bind:value={repoLocalPath}
							/>
						</div>

						<div class="space-y-1.5">
							<label for="repo-setup" class="text-sm font-medium text-foreground">Setup Script</label>
							<p class="text-xs text-muted-foreground">Runs when initializing a new worktree</p>
							<textarea
								id="repo-setup"
								class="w-full h-24 bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-y font-mono"
								placeholder="npm install && npm run build"
								bind:value={repoSetupScript}
							></textarea>
						</div>

						<div class="space-y-1.5">
							<label for="repo-run" class="text-sm font-medium text-foreground">Run Script</label>
							<p class="text-xs text-muted-foreground">Command to run tests</p>
							<textarea
								id="repo-run"
								class="w-full h-24 bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-y font-mono"
								placeholder="npm test"
								bind:value={repoRunScript}
							></textarea>
						</div>

						<div class="grid grid-cols-2 gap-4">
							<div class="space-y-1.5">
								<label for="repo-base-branch" class="text-sm font-medium text-foreground">Base Branch</label>
								<input
									id="repo-base-branch"
									class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
									placeholder="main"
									bind:value={repoBaseBranch}
								/>
							</div>
							<div class="space-y-1.5">
								<label for="repo-branch-prefix" class="text-sm font-medium text-foreground">Branch Prefix</label>
								<input
									id="repo-branch-prefix"
									class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
									placeholder="autodev/"
									bind:value={repoBranchPrefix}
								/>
							</div>
						</div>

						<div class="space-y-1.5">
							<label for="repo-worktree" class="text-sm font-medium text-foreground">Worktree Directory</label>
							<p class="text-xs text-muted-foreground">Directory for git worktrees</p>
							<input
								id="repo-worktree"
								class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring font-mono"
								placeholder="/tmp/autodev-worktrees"
								bind:value={repoWorktreeDir}
							/>
						</div>

						<div class="flex justify-end pt-2">
							<button
								class="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
								onclick={async () => {
									if (!selectedRepo) return;
									// Save repo local path
									if (repoLocalPath.trim()) {
										await backend.setRepoPath(selectedRepo.id, repoLocalPath.trim());
									}
									// Save repo config
									await backend.updateRepo({
										...selectedRepo,
										setup_script: repoSetupScript,
										run_script: repoRunScript,
										base_branch: repoBaseBranch,
										branch_prefix: repoBranchPrefix,
										worktree_dir: repoWorktreeDir,
									});
								}}
							>
								Save Repository Settings
							</button>
						</div>
					{/if}
				</Tabs.Content>
			</Tabs.Root>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
