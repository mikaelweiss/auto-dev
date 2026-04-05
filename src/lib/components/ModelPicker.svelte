<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { ChevronDown, Check } from 'lucide-svelte';
	import {
		MODEL_REGISTRY,
		PROVIDER_LABELS,
		getModelInfo,
		type ProviderKind
	} from '$lib/types';

	interface Props {
		value: string;
		onSelect: (modelId: string) => void;
		compact?: boolean;
	}

	let { value, onSelect, compact = false }: Props = $props();

	let open = $state(false);

	let currentModel = $derived(getModelInfo(value));

	let providers = $derived(
		[...new Set(MODEL_REGISTRY.map((m) => m.provider))] as ProviderKind[]
	);

	function select(modelId: string) {
		onSelect(modelId);
		open = false;
	}
</script>

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger
		class="flex items-center gap-1 rounded-md border border-border bg-muted/50 text-foreground transition-colors hover:bg-muted
			{compact ? 'px-2 py-1 text-xs' : 'px-3 py-1.5 text-sm'}"
	>
		<span class="truncate">{currentModel?.display_name ?? value}</span>
		<ChevronDown class="{compact ? 'h-3 w-3' : 'h-3.5 w-3.5'} shrink-0 text-muted-foreground" />
	</DropdownMenu.Trigger>

	<DropdownMenu.Portal>
		<DropdownMenu.Content
			class="z-[100] min-w-[200px] rounded-lg border border-border bg-popover shadow-lg animate-in fade-in-0 zoom-in-95"
			sideOffset={4}
			side="top"
		>
			<div class="max-h-64 overflow-y-auto py-1">
				{#each providers as provider (provider)}
					<div class="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
						{PROVIDER_LABELS[provider]}
					</div>
					{#each MODEL_REGISTRY.filter((m) => m.provider === provider) as model (model.id)}
						<DropdownMenu.Item
							class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors outline-none cursor-pointer
								{model.id === value
								? 'bg-muted text-foreground'
								: 'text-muted-foreground hover:bg-muted/50 hover:text-foreground data-[highlighted]:bg-muted/50 data-[highlighted]:text-foreground'}"
							onSelect={() => select(model.id)}
						>
							<span class="flex-1 truncate">{model.display_name}</span>
							{#if model.id === value}
								<Check class="h-3.5 w-3.5 shrink-0 text-primary" />
							{/if}
						</DropdownMenu.Item>
					{/each}
				{/each}
			</div>
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>
