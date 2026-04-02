<script lang="ts">
	import type { Issue, ColumnId } from '$lib/types';
	import { COLUMN_ORDER, COLUMN_CONFIG, getColumnForIssue } from '$lib/types';
	import { issuesByColumn, refreshIssues } from '$lib/stores/issues';
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

	// Suppress store sync while an API call is in flight
	let actionInFlight = $state(false);

	// Sync from store when it updates (unless we're mid-action)
	$effect(() => {
		const storeData = $issuesByColumn;
		if (actionInFlight) return;
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
				moveIssueToColumn(columnId, droppedIssue as Issue);
			}
		}
	}

	async function moveIssueToColumn(targetColumn: ColumnId, issue: Issue) {
		const currentColumn = getColumnForIssue(issue);
		if (currentColumn === targetColumn) return;

		const currentLabel = COLUMN_CONFIG[currentColumn].github_label;
		const targetLabel = COLUMN_CONFIG[targetColumn].github_label;

		actionInFlight = true;
		try {
			// Remove old label if it has one
			if (currentLabel) {
				await backend.removeLabel(issue.repo_owner, issue.repo_name, issue.number, currentLabel);
			}

			// Add new label if the target column has one
			if (targetLabel) {
				await backend.addLabel(issue.repo_owner, issue.repo_name, issue.number, targetLabel);
			}

			// Handle special cases for "done" column
			if (targetColumn === 'done') {
				if (issue.pull_request) {
					const prNumber = parseInt(issue.pull_request.url.split('/').pop() ?? '0', 10);
					if (prNumber) {
						await backend.mergePR(issue.repo_owner, issue.repo_name, prNumber);
					}
				} else {
					await backend.closeIssue(issue.repo_owner, issue.repo_name, issue.number);
				}
			}

			// Refresh from GitHub to get the confirmed state
			await refreshIssues(issue.repo_owner, issue.repo_name);
		} catch (e) {
			console.error('Failed to move issue:', e);
			// Refresh to revert visual state to what GitHub actually has
			await refreshIssues(issue.repo_owner, issue.repo_name);
		} finally {
			actionInFlight = false;
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
