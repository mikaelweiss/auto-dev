<script lang="ts">
	import { showKeyboardShortcuts } from '$lib/stores/ui';
	import { X } from 'lucide-svelte';

	const groups = [
		{
			title: 'General',
			shortcuts: [
				{ keys: ['⌘', 'K'], label: 'Command palette' },
				{ keys: ['⌘', 'N'], label: 'New issue' },
				{ keys: ['⌘', ','], label: 'Toggle settings' },
				{ keys: ['⌘', '/'], label: 'Keyboard shortcuts' },
				{ keys: ['Esc'], label: 'Close overlay' }
			]
		},
		{
			title: 'Board',
			shortcuts: [
				{ keys: ['1'], label: 'Jump to Backlog' },
				{ keys: ['2'], label: 'Jump to Planning' },
				{ keys: ['3'], label: 'Jump to In Progress' },
				{ keys: ['4'], label: 'Jump to Blocked' },
				{ keys: ['5'], label: 'Jump to Review' },
				{ keys: ['6'], label: 'Jump to Done' }
			]
		},
		{
			title: 'Repositories',
			shortcuts: [
				{ keys: ['⌃', '1-9'], label: 'Switch repository' }
			]
		}
	];

	function close() {
		showKeyboardShortcuts.set(false);
	}

	function handleOverlayClick(e: MouseEvent) {
		if (e.target === e.currentTarget) close();
	}
</script>

{#if $showKeyboardShortcuts}
	<!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
	<div
		class="fixed inset-0 z-[60] flex items-center justify-center"
		onclick={handleOverlayClick}
	>
		<div class="absolute inset-0 bg-black/40 backdrop-blur-sm"></div>

		<div class="relative w-full max-w-md bg-popover border border-border rounded-xl shadow-2xl overflow-hidden">
			<div class="flex items-center justify-between px-5 py-4 border-b border-border">
				<h2 class="text-sm font-semibold text-foreground">Keyboard Shortcuts</h2>
				<button
					class="p-1 rounded-md hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
					onclick={close}
				>
					<X class="h-4 w-4" />
				</button>
			</div>

			<div class="px-5 py-4 space-y-5 max-h-[70vh] overflow-y-auto">
				{#each groups as group}
					<div class="space-y-2">
						<h3 class="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
							{group.title}
						</h3>
						{#each group.shortcuts as shortcut}
							<div class="flex items-center justify-between py-1">
								<span class="text-sm text-foreground">{shortcut.label}</span>
								<div class="flex items-center gap-1">
									{#each shortcut.keys as key}
										<kbd class="min-w-[24px] px-1.5 py-0.5 rounded bg-muted text-[11px] font-mono text-muted-foreground text-center border border-border/50">
											{key}
										</kbd>
									{/each}
								</div>
							</div>
						{/each}
					</div>
				{/each}
			</div>
		</div>
	</div>
{/if}
