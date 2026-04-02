<script lang="ts">
	import type { Issue, ColumnId } from '$lib/types';
	import { COLUMN_ORDER, COLUMN_CONFIG, getColumnForIssue } from '$lib/types';
	import { issuesByColumn, refreshIssues } from '$lib/stores/issues';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import { log } from '$lib/stores/backend';
	import { type DndEvent, TRIGGERS } from 'svelte-dnd-action';
	import KanbanColumn from './KanbanColumn.svelte';

	let errorMessage = $state('');

	// Local mutable copy of issues by column so DnD can update it optimistically
	let columns = $state<Record<ColumnId, Issue[]>>({
		backlog: [],
		claimed: [],
		in_progress: [],
		blocked: [],
		review: [],
		done: []
	});

	// Sync from store
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
				moveIssueToColumn(columnId, droppedIssue as Issue);
			}
		}
	}

	async function moveIssueToColumn(targetColumn: ColumnId, issue: Issue) {
		const currentColumn = getColumnForIssue(issue);
		const repo = $repos.find(
			(r) => r.owner === issue.repo_owner && r.name === issue.repo_name
		);

		log('DRAG', `moveIssueToColumn: #${issue.number} "${issue.title}" from=${currentColumn} to=${targetColumn} repo=${repo?.id ?? 'NOT FOUND'}`);

		if (targetColumn === 'claimed' && repo) {
			log('DRAG', `Starting session for repo_id=${repo.id} issue=${issue.number}`);
			try {
				const result = await backend.startSession(repo.id, issue.number);
				log('DRAG', `startSession resolved: ${JSON.stringify(result)}`);
			} catch (e) {
				log('DRAG', `startSession THREW: ${e}`);
				errorMessage = String(e);
				setTimeout(() => { errorMessage = ''; }, 8000);
			}
			log('DRAG', `Syncing label from=${currentColumn} to=${targetColumn}`);
			syncLabel(issue, currentColumn, targetColumn);
			return;
		}

		if (targetColumn === 'done') {
			if (issue.pull_request) {
				const prNumber = parseInt(issue.pull_request.url.split('/').pop() ?? '0', 10);
				if (prNumber) {
					try {
						await backend.mergePR(issue.repo_owner, issue.repo_name, prNumber);
					} catch (e) {
						console.error('Failed to merge PR:', e);
					}
				}
			} else {
				try {
					await backend.closeIssue(issue.repo_owner, issue.repo_name, issue.number);
				} catch (e) {
					console.error('Failed to close issue:', e);
				}
			}
			syncLabel(issue, currentColumn, targetColumn);
			await refreshIssues(issue.repo_owner, issue.repo_name);
			return;
		}

		// For other column moves (no session involved), update labels and refresh
		syncLabel(issue, currentColumn, targetColumn);
		await refreshIssues(issue.repo_owner, issue.repo_name);
	}

	/** Best-effort label sync to GitHub — fire and forget. */
	function syncLabel(issue: Issue, fromColumn: ColumnId, toColumn: ColumnId) {
		const oldLabel = COLUMN_CONFIG[fromColumn].github_label;
		const newLabel = COLUMN_CONFIG[toColumn].github_label;
		log('LABEL', `syncLabel: #${issue.number} oldLabel=${oldLabel} newLabel=${newLabel}`);

		if (oldLabel) {
			backend
				.removeLabel(issue.repo_owner, issue.repo_name, issue.number, oldLabel)
				.then(() => log('LABEL', `Removed label "${oldLabel}" from #${issue.number}`))
				.catch((e) => log('LABEL', `FAILED to remove label "${oldLabel}": ${e}`));
		}
		if (newLabel) {
			backend
				.addLabel(issue.repo_owner, issue.repo_name, issue.number, newLabel)
				.then(() => log('LABEL', `Added label "${newLabel}" to #${issue.number}`))
				.catch((e) => log('LABEL', `FAILED to add label "${newLabel}": ${e}`));
		}
	}
</script>

<div class="flex-1 flex flex-col min-h-0 relative">
	{#if errorMessage}
		<div class="mx-4 mt-2 px-3 py-2 rounded-md bg-red-950/80 border border-red-800 text-red-300 text-xs">
			<button class="float-right ml-2 text-red-400 hover:text-red-200" onclick={() => { errorMessage = ''; }}>x</button>
			{errorMessage}
		</div>
	{/if}
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
</div>
