<script lang="ts">
	import { Brain, Send, Square } from 'lucide-svelte';
	import * as backend from '$lib/stores/backend';
	import type { Session, Issue, RepoConfig } from '$lib/types';
	import { getModelInfo } from '$lib/types';
	import ModelPicker from './ModelPicker.svelte';

	interface Props {
		session: Session | null;
		issue: Issue;
		repoConfig: RepoConfig | null;
		onNewSession: (message?: string, model?: string, effort?: string) => void;
		onTest: () => void;
		onMerge: () => void;
		onCopy: () => void;
	}

	let { session, issue, repoConfig, onNewSession, onTest, onMerge, onCopy }: Props = $props();

	let selectedModel = $state('claude-sonnet-4-6');
	let selectedEffort = $state('high');

	let effortLevels = $derived(getModelInfo(selectedModel)?.effort_levels ?? ['low', 'medium', 'high', 'max']);

	const EFFORT_LABELS: Record<string, string> = {
		low: 'Low',
		medium: 'Medium',
		high: 'High',
		max: 'Extra high'
	};

	function handleModelSelect(modelId: string) {
		selectedModel = modelId;
		const info = getModelInfo(modelId);
		if (info && !info.effort_levels.includes(selectedEffort)) {
			selectedEffort = info.default_effort;
		}
	}

	function cycleEffort() {
		const idx = effortLevels.indexOf(selectedEffort);
		selectedEffort = effortLevels[(idx + 1) % effortLevels.length];
	}

	let input = $state('');
	let textareaEl: HTMLTextAreaElement | undefined = $state(undefined);
	let showSlashMenu = $state(false);
	let showAtMenu = $state(false);
	let slashFilter = $state('');
	let atFilter = $state('');
	let selectedMenuIndex = $state(0);
	let files: string[] = $state([]);
	let filesLoading = $state(false);
	let filesError = $state('');
	let menuContainer: HTMLDivElement | undefined = $state(undefined);

	let isRunning = $derived(
		session?.status === 'running' ||
			session?.status === 'initializing' ||
			session?.status === 'setup'
	);
	let canSend = $derived(!isRunning && input.trim().length > 0);

	const slashCommands = [
		{ name: 'test', description: 'Run tests for this session' },
		{ name: 'merge', description: 'Squash merge the PR' },
		{ name: 'stop', description: 'Stop the running session' },
		{ name: 'new', description: 'Start a new session' },
		{ name: 'copy', description: 'Copy conversation to clipboard' }
	];

	let filteredCommands = $derived(
		slashCommands.filter((c) => c.name.toLowerCase().includes(slashFilter.toLowerCase()))
	);

	let filteredFiles = $derived(
		files
			.filter((f) => f.toLowerCase().includes(atFilter.toLowerCase()))
			.slice(0, 15)
	);

	function autoResize() {
		if (textareaEl) {
			textareaEl.style.height = 'auto';
			textareaEl.style.height = Math.min(textareaEl.scrollHeight, 160) + 'px';
		}
	}

	function handleInput() {
		autoResize();

		// Slash commands: only when input starts with /
		if (input.startsWith('/')) {
			const rest = input.slice(1);
			if (!rest.includes(' ')) {
				showSlashMenu = true;
				showAtMenu = false;
				slashFilter = rest;
				selectedMenuIndex = 0;
				return;
			}
		}

		// @ file picker
		const cursorPos = textareaEl?.selectionStart ?? input.length;
		const textBeforeCursor = input.slice(0, cursorPos);
		const lastAt = textBeforeCursor.lastIndexOf('@');
		if (lastAt >= 0) {
			const afterAt = textBeforeCursor.slice(lastAt + 1);
			if (!afterAt.includes(' ') && !afterAt.includes('\n')) {
				showAtMenu = true;
				showSlashMenu = false;
				atFilter = afterAt;
				selectedMenuIndex = 0;
				if (files.length === 0 && !filesLoading) {
					loadFiles();
				}
				return;
			}
		}

		showSlashMenu = false;
		showAtMenu = false;
	}

	async function loadFiles() {
		if (!session) return;
		filesLoading = true;
		filesError = '';
		try {
			files = await backend.listSessionFiles(session.id);
		} catch {
			files = [];
			filesError = 'Failed to load files';
		}
		filesLoading = false;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (showSlashMenu || showAtMenu) {
			const items = showSlashMenu ? filteredCommands : filteredFiles;
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				selectedMenuIndex = Math.max(0, selectedMenuIndex - 1);
				scrollMenuItemIntoView();
				return;
			}
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				selectedMenuIndex = Math.min(items.length - 1, selectedMenuIndex + 1);
				scrollMenuItemIntoView();
				return;
			}
			if (e.key === 'Tab' || e.key === 'Enter') {
				e.preventDefault();
				if (showSlashMenu && filteredCommands.length > 0) {
					executeSlashCommand(filteredCommands[selectedMenuIndex].name);
				} else if (showAtMenu && filteredFiles.length > 0) {
					insertFileReference(filteredFiles[selectedMenuIndex]);
				}
				return;
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				showSlashMenu = false;
				showAtMenu = false;
				return;
			}
		}

		// Enter to send (without shift)
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	function scrollMenuItemIntoView() {
		queueMicrotask(() => {
			if (menuContainer) {
				const selected = menuContainer.querySelector('[data-selected="true"]');
				selected?.scrollIntoView({ block: 'nearest' });
			}
		});
	}

	function executeSlashCommand(name: string) {
		showSlashMenu = false;
		input = '';
		autoResize();

		switch (name) {
			case 'test':
				onTest();
				break;
			case 'merge':
				onMerge();
				break;
			case 'stop':
				handleStop();
				break;
			case 'new':
				onNewSession();
				break;
			case 'copy':
				onCopy();
				break;
		}
	}

	function insertFileReference(filePath: string) {
		const cursorPos = textareaEl?.selectionStart ?? input.length;
		const textBeforeCursor = input.slice(0, cursorPos);
		const lastAt = textBeforeCursor.lastIndexOf('@');
		const before = input.slice(0, lastAt);
		const after = input.slice(cursorPos);
		input = `${before}@${filePath} ${after}`;
		showAtMenu = false;
		autoResize();
		// Focus and move cursor after inserted text
		queueMicrotask(() => {
			if (textareaEl) {
				const newPos = before.length + 1 + filePath.length + 1;
				textareaEl.focus();
				textareaEl.setSelectionRange(newPos, newPos);
			}
		});
	}

	async function handleSend() {
		if (!canSend) return;
		const message = input.trim();
		input = '';
		autoResize();
		if (session) {
			await backend.respondToSession(session.id, message, selectedModel, selectedEffort);
		} else {
			onNewSession(message, selectedModel, selectedEffort);
		}
	}

	async function handleStop() {
		if (!session || !isRunning) return;
		await backend.stopSession(session.id);
	}
</script>

<div class="border-t border-border bg-background">
	<!-- Slash command dropdown -->
	{#if showSlashMenu && filteredCommands.length > 0}
		<div
			bind:this={menuContainer}
			class="border-b border-border max-h-48 overflow-y-auto"
		>
			{#each filteredCommands as cmd, i (cmd.name)}
				<button
					type="button"
					class="w-full flex items-center gap-3 px-4 py-2 text-left text-sm transition-colors
						{i === selectedMenuIndex ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
					data-selected={i === selectedMenuIndex}
					onclick={() => executeSlashCommand(cmd.name)}
					onmouseenter={() => {
						selectedMenuIndex = i;
					}}
				>
					<span class="text-muted-foreground font-mono text-xs">/{cmd.name}</span>
					<span class="text-xs text-muted-foreground/70">{cmd.description}</span>
				</button>
			{/each}
		</div>
	{/if}

	<!-- @ file picker dropdown -->
	{#if showAtMenu && (filteredFiles.length > 0 || filesLoading)}
		<div
			bind:this={menuContainer}
			class="border-b border-border max-h-48 overflow-y-auto"
		>
			{#if filesLoading}
				<div class="px-4 py-2 text-xs text-muted-foreground">Loading files...</div>
			{:else if filesError}
				<div class="px-4 py-2 text-xs text-red-400">{filesError}</div>
			{:else}
				{#each filteredFiles as file, i (file)}
					<button
						type="button"
						class="w-full flex items-center gap-2 px-4 py-1.5 text-left text-xs font-mono transition-colors
							{i === selectedMenuIndex ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
						data-selected={i === selectedMenuIndex}
						onclick={() => insertFileReference(file)}
						onmouseenter={() => {
							selectedMenuIndex = i;
						}}
					>
						{file}
					</button>
				{/each}
			{/if}
		</div>
	{/if}

	<!-- Input area -->
	<div class="flex items-end gap-2 p-3">
		<textarea
			bind:this={textareaEl}
			bind:value={input}
			oninput={handleInput}
			onkeydown={handleKeydown}
			placeholder={isRunning ? 'Agent is working...' : 'Send a message... (/ for commands, @ for files)'}
			rows="1"
			class="flex-1 bg-muted rounded-lg px-3 py-2 text-sm text-foreground border border-border outline-none focus:ring-1 focus:ring-ring resize-none min-h-[36px] max-h-[160px]"
		></textarea>

		{#if isRunning}
			<button
				class="shrink-0 p-2 rounded-lg bg-red-600 hover:bg-red-500 text-white transition-colors"
				onclick={handleStop}
				title="Stop session"
			>
				<Square class="h-4 w-4" />
			</button>
		{:else}
			<button
				class="shrink-0 p-2 rounded-lg bg-primary text-primary-foreground transition-colors disabled:opacity-30 disabled:cursor-not-allowed hover:bg-primary/90"
				onclick={handleSend}
				disabled={!canSend}
				title="Send message (Enter)"
			>
				<Send class="h-4 w-4" />
			</button>
		{/if}
	</div>

	<!-- Model picker & hint bar -->
	<div class="flex items-center justify-between px-4 pb-2 -mt-1">
		<div class="flex items-center gap-2">
			<ModelPicker
				value={selectedModel}
				onSelect={handleModelSelect}
				compact
			/>
			<button
				type="button"
				class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
				onclick={cycleEffort}
				title="Thinking level (click to cycle)"
			>
				<Brain class="h-3.5 w-3.5" />
				<span class="text-[10px] text-muted-foreground/60">:</span>
				<span>{EFFORT_LABELS[selectedEffort] ?? selectedEffort}</span>
			</button>
		</div>
		<span class="text-[10px] text-muted-foreground/50">
			<kbd class="px-1 py-0.5 rounded bg-muted/60 text-[9px]">Enter</kbd> send
			<kbd class="px-1 py-0.5 rounded bg-muted/60 text-[9px] ml-1">Shift+Enter</kbd> newline
			<kbd class="px-1 py-0.5 rounded bg-muted/60 text-[9px] ml-1">/</kbd> commands
			<kbd class="px-1 py-0.5 rounded bg-muted/60 text-[9px] ml-1">@</kbd> files
		</span>
	</div>
</div>
