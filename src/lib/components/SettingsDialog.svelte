<script lang="ts">
	import { untrack } from 'svelte';
	import { Dialog, Tabs, Switch } from 'bits-ui';
	import { showSettings } from '$lib/stores/ui';
	import { appSettings, agentPrompts } from '$lib/stores/settings';
	import { repos, selectedRepoId } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import type { SessionStage, AppSettings, ProviderKind } from '$lib/types';
	import { getModelInfo, getProviderForModel } from '$lib/types';
	import { X } from 'lucide-svelte';
	import ModelPicker from './ModelPicker.svelte';

	// Local copies of settings for editing
	let localSettings = $state<AppSettings>({
		sleep_prevention: true,
		notifications_enabled: true,
		poll_interval_seconds: 15,
		bypass_permissions: false
	});

	// Local copies of prompts (per-stage text, model, effort)
	let localPrompts = $state<Record<SessionStage, string>>({
		spec: '',
		implement: '',
		review: '',
		ci_fix: '',
		merge_conflict: ''
	});

	let localModels = $state<Record<SessionStage, string>>({
		spec: 'claude-sonnet-4-6',
		implement: 'claude-sonnet-4-6',
		review: 'claude-sonnet-4-6',
		ci_fix: 'claude-sonnet-4-6',
		merge_conflict: 'claude-sonnet-4-6'
	});

	let localEfforts = $state<Record<SessionStage, string>>({
		spec: 'high',
		implement: 'high',
		review: 'high',
		ci_fix: 'high',
		merge_conflict: 'high'
	});

	// Repo-specific settings
	let repoLocalPath = $state('');
	let repoSetupScript = $state('');
	let repoRunScript = $state('');
	let repoBaseBranch = $state('');
	let repoBranchPrefix = $state('');

	let selectedRepo = $derived($repos.find((r) => r.id === $selectedRepoId));

	const STAGE_LABELS: Record<SessionStage, string> = {
		spec: 'Spec',
		implement: 'Implement',
		review: 'Review',
		ci_fix: 'CI Fix',
		merge_conflict: 'Merge Conflict'
	};

	const STAGES: SessionStage[] = ['spec', 'implement', 'review', 'ci_fix', 'merge_conflict'];

	// Debounce helper
	let debounceTimers: Record<string, ReturnType<typeof setTimeout>> = {};
	function debounce(key: string, fn: () => void, ms = 500) {
		clearTimeout(debounceTimers[key]);
		debounceTimers[key] = setTimeout(fn, ms);
	}

	// Sync local state when dialog opens — only $showSettings is tracked.
	// Fetches fresh data from backend and populates local state directly.
	$effect(() => {
		if (!$showSettings) return;

		untrack(() => {
			// Fetch fresh settings from backend and populate local state
			backend.getSettings().then((s) => {
				localSettings = { ...s };
				appSettings.set(s);
			});

			backend.getPrompts().then((prompts) => {
				agentPrompts.set(prompts);
				for (const prompt of prompts) {
					localPrompts[prompt.stage] = prompt.prompt_text;
					localModels[prompt.stage] = prompt.model;
					localEfforts[prompt.stage] = prompt.effort;
				}
			});

			const repo = selectedRepo;
			if (repo) {
				repoSetupScript = repo.setup_script;
				repoRunScript = repo.run_script;
				repoBaseBranch = repo.base_branch;
				repoBranchPrefix = repo.branch_prefix;

				backend.getRepoPath(repo.id).then((path) => {
					repoLocalPath = path ?? '';
				});
			}
		});
	});

	function handleClose() {
		// Sync local edits back to stores on close
		appSettings.set({ ...localSettings });
		showSettings.set(false);
	}

	// Auto-save: persist app settings to backend
	function persistAppSettings() {
		backend.updateSettings({ ...localSettings });
	}

	// Auto-save: persist a single prompt stage
	function persistPrompt(stage: SessionStage) {
		const provider = getProviderForModel(localModels[stage]);
		backend.updatePrompt(stage, localPrompts[stage], provider, localModels[stage], localEfforts[stage]);
	}

	// Auto-save: persist repo settings
	function persistRepoSettings() {
		if (!selectedRepo) return;
		if (repoLocalPath.trim()) {
			backend.setRepoPath(selectedRepo.id, repoLocalPath.trim());
		}
		backend.updateRepo({
			...selectedRepo,
			setup_script: repoSetupScript,
			run_script: repoRunScript,
			base_branch: repoBaseBranch,
			branch_prefix: repoBranchPrefix
		});
	}
</script>

{#snippet segmentedButtons(options: string[], value: string, onSelect: (v: string) => void)}
	<div class="inline-flex rounded-md border border-border overflow-hidden">
		{#each options as option (option)}
			<button
				class="px-2.5 py-1 text-xs font-medium transition-colors {value === option
					? 'bg-primary text-primary-foreground'
					: 'bg-muted text-muted-foreground hover:bg-muted/80 hover:text-foreground'}"
				onclick={() => onSelect(option)}
			>
				{option}
			</button>
		{/each}
	</div>
{/snippet}

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
						Agent Config
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
								onCheckedChange={(checked) => { localSettings.sleep_prevention = checked; persistAppSettings(); }}
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
								onCheckedChange={(checked) => { localSettings.notifications_enabled = checked; persistAppSettings(); }}
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
								oninput={() => debounce('poll-interval', persistAppSettings)}
							/>
						</div>
					</div>

					<div class="border-t border-border pt-4 mt-4 space-y-3">
						<div>
							<p class="text-sm font-medium text-foreground">Agent Permissions</p>
							<p class="text-xs text-muted-foreground">Controls how much autonomy agents have when running tasks</p>
						</div>

						<div class="flex items-center justify-between">
							<div>
								<p class="text-sm font-medium text-foreground">Bypass All Permissions</p>
								<p class="text-xs text-muted-foreground">
									{#if localSettings.bypass_permissions}
										Agents run with no permission checks. Use only in trusted repos.
									{:else}
										Agents use auto mode — an AI classifier approves safe actions.
									{/if}
								</p>
							</div>
							<Switch.Root
								checked={localSettings.bypass_permissions}
								onCheckedChange={(checked) => { localSettings.bypass_permissions = checked; persistAppSettings(); }}
								class="relative h-5 w-9 rounded-full bg-muted transition-colors data-[state=checked]:bg-destructive"
							>
								<Switch.Thumb class="block h-4 w-4 rounded-full bg-background shadow-sm transition-transform data-[state=checked]:translate-x-4 translate-x-0.5" />
							</Switch.Root>
						</div>
					</div>

				</Tabs.Content>

				<!-- Agent Config Tab -->
				<Tabs.Content value="prompts" class="flex-1 overflow-y-auto p-4 space-y-4">
					{#each STAGES as stage (stage)}
						{@const modelInfo = getModelInfo(localModels[stage])}
						{@const effortLevels = modelInfo?.effort_levels ?? ['low', 'medium', 'high', 'max']}
						<div class="rounded-lg border border-border p-3 space-y-3">
							<div class="flex items-center justify-between">
								<p class="text-sm font-semibold text-foreground">{STAGE_LABELS[stage]}</p>
								<div class="flex items-center gap-3">
									<div class="flex items-center gap-1.5">
										<span class="text-xs text-muted-foreground">Model</span>
										<ModelPicker
											value={localModels[stage]}
											onSelect={(v) => {
												localModels[stage] = v;
												// Reset effort to default if current effort isn't valid for new model
												const info = getModelInfo(v);
												if (info && !info.effort_levels.includes(localEfforts[stage])) {
													localEfforts[stage] = info.default_effort;
												}
												persistPrompt(stage);
											}}
											compact
										/>
									</div>
									<div class="flex items-center gap-1.5">
										<span class="text-xs text-muted-foreground">Effort</span>
										{@render segmentedButtons(effortLevels, localEfforts[stage], (v) => { localEfforts[stage] = v; persistPrompt(stage); })}
									</div>
								</div>
							</div>
							<textarea
								id="prompt-{stage}"
								class="w-full h-24 bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-y font-mono"
								placeholder="Custom prompt for {STAGE_LABELS[stage]} stage..."
								bind:value={localPrompts[stage]}
								oninput={() => debounce(`prompt-${stage}`, () => persistPrompt(stage))}
							></textarea>
						</div>
					{/each}

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
								oninput={() => debounce('repo-path', persistRepoSettings)}
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
								oninput={() => debounce('repo-setup', persistRepoSettings)}
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
								oninput={() => debounce('repo-run', persistRepoSettings)}
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
									oninput={() => debounce('repo-base', persistRepoSettings)}
								/>
							</div>
							<div class="space-y-1.5">
								<label for="repo-branch-prefix" class="text-sm font-medium text-foreground">Branch Prefix</label>
								<input
									id="repo-branch-prefix"
									class="w-full bg-muted rounded-md px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring"
									placeholder="autodev/"
									bind:value={repoBranchPrefix}
									oninput={() => debounce('repo-prefix', persistRepoSettings)}
								/>
							</div>
						</div>

					{/if}
				</Tabs.Content>
			</Tabs.Root>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
