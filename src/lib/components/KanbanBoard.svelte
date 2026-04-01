<script lang="ts">
	import type { Issue, ColumnId } from '$lib/types';
	import { COLUMN_ORDER, COLUMN_CONFIG } from '$lib/types';
	import { issuesByColumn } from '$lib/stores/issues';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import { type DndEvent, TRIGGERS } from 'svelte-dnd-action';
	import KanbanColumn from './KanbanColumn.svelte';

	// Local mutable copy of issues by column so DnD can update it optimistically
	let columns = $state<Record<ColumnId, Issue[]>>({
		backlog: [],
		claimed: [],
		in_progress: [],
		blocked: [],
		review: [],
		done: []
	});

	// Sync from store when it updates
	$effect(() => {
		const storeData = $issuesByColumn;
		columns = {
			backlog: [...storeData.backlog],
			claimed: [...storeData.claimed],
			in_progress: [...storeData.in_progress],
			blocked: [...storeData.blocked],
			review: [...storeData.review],
			done: [...storeData.done]
		};
	});

	function handleConsider(columnId: ColumnId, e: CustomEvent<DndEvent<Issue>>) {
		columns[columnId] = e.detail.items as Issue[];
	}

	function handleFinalize(columnId: ColumnId, e: CustomEvent<DndEvent<Issue>>) {
		columns[columnId] = e.detail.items as Issue[];

		const { info } = e.detail;
		if (info.trigger === TRIGGERS.DROPPED_INTO_ZONE || info.trigger === TRIGGERS.DROPPED_INTO_ANOTHER) {
			const droppedIssue = e.detail.items.find((item) => String(item.id) === info.id);
			if (droppedIssue) {
				dispatchColumnAction(columnId, droppedIssue as Issue);
			}
		}
	}

	function dispatchColumnAction(targetColumn: ColumnId, issue: Issue) {
		const repo = $repos.find((r) => r.owner === issue.repo_owner && r.name === issue.repo_name);
		if (!repo) return;

		switch (targetColumn) {
			case 'claimed':
				// Start session (spec stage)
				backend.startSession(repo.id, issue.number);
				break;
			case 'in_progress':
				// Also start a session if not already running
				backend.startSession(repo.id, issue.number);
				break;
			case 'backlog':
				// No special action, just moved back
				break;
			case 'blocked':
				// No automatic action
				break;
			case 'review':
				// No automatic action
				break;
			case 'done':
				// If there's a PR, attempt merge
				if (issue.pull_request) {
					const prNumber = parseInt(issue.pull_request.url.split('/').pop() ?? '0', 10);
					if (prNumber) {
						backend.mergePR(issue.repo_owner, issue.repo_name, prNumber);
					}
				}
				break;
		}
	}
</script>

<div class="flex-1 flex gap-3 overflow-x-auto px-4 py-3 min-h-0">
	{#each COLUMN_ORDER as colId (colId)}
		<KanbanColumn
			columnId={colId}
			title={COLUMN_CONFIG[colId].label}
			issues={columns[colId]}
			onconsider={handleConsider}
			onfinalize={handleFinalize}
		/>
	{/each}
</div>
