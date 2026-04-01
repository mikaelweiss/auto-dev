<script lang="ts">
	import type { Issue, ColumnId } from '$lib/types';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import IssueCard from './IssueCard.svelte';

	let {
		columnId,
		title,
		issues,
		onconsider,
		onfinalize
	}: {
		columnId: ColumnId;
		title: string;
		issues: Issue[];
		onconsider: (columnId: ColumnId, e: CustomEvent<DndEvent<Issue>>) => void;
		onfinalize: (columnId: ColumnId, e: CustomEvent<DndEvent<Issue>>) => void;
	} = $props();

	const flipDurationMs = 200;
</script>

<div class="flex flex-col min-w-[280px] w-[280px] max-w-[320px] h-full">
	<div class="flex items-center justify-between px-3 py-2 mb-2">
		<h3 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{title}</h3>
		<span class="text-xs font-medium text-muted-foreground bg-muted rounded-full px-2 py-0.5">
			{issues.length}
		</span>
	</div>

	<div
		class="flex-1 overflow-y-auto space-y-2 px-1 pb-2 rounded-lg min-h-[60px]"
		use:dndzone={{
			items: issues,
			type: 'kanban',
			flipDurationMs,
			dropTargetClasses: ['outline', 'outline-2', 'outline-ring/40', 'outline-offset-[-2px]', 'rounded-lg', 'bg-accent/30']
		}}
		onconsider={(e) => onconsider(columnId, e)}
		onfinalize={(e) => onfinalize(columnId, e)}
	>
		{#each issues as issue (issue.id)}
			<div animate:flip={{ duration: flipDurationMs }}>
				<IssueCard {issue} />
			</div>
		{/each}
	</div>
</div>
