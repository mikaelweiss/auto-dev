<script lang="ts">
	import type { Issue, ColumnId, RepoConfig } from '$lib/types';
	import { COLUMN_ORDER, COLUMN_CONFIG } from '$lib/types';
	import { issuesByColumn, issueStates, refreshIssues } from '$lib/stores/issues';
	import { repos } from '$lib/stores/repos';
	import * as backend from '$lib/stores/backend';
	import { type DndEvent, TRIGGERS } from 'svelte-dnd-action';
	import KanbanColumn from './KanbanColumn.svelte';

	let errorMessage = $state('');

	// Local mutable copy of issues by column so DnD can update it optimistically
	let columns = $state<Record<ColumnId, Issue[]>>({
		backlog: [],
		planning: [],
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
			planning: [...storeData.planning],
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
			const droppedIssue = e.detail.items.find((item) => String(item.id) === String(info.id));
			if (droppedIssue) {
				moveIssueToColumn(columnId, droppedIssue as Issue);
			}
		}
	}

	async function moveIssueToColumn(targetColumn: ColumnId, issue: Issue) {
		const repo = $repos.find(
			(r) => r.owner === issue.repo_owner && r.name === issue.repo_name
		);

		if (targetColumn === 'planning' && repo) {
			try {
				await backend.startSession(repo.id, issue.number);
			} catch (e) {
				errorMessage = String(e);
				setTimeout(() => { errorMessage = ''; }, 8000);
			}
			saveColumnState(repo, issue, targetColumn);
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
			saveColumnState(repo, issue, targetColumn);
			await refreshIssues(issue.repo_owner, issue.repo_name);
			return;
		}

		saveColumnState(repo, issue, targetColumn);
	}

	function saveColumnState(repo: RepoConfig | undefined, issue: Issue, columnId: ColumnId) {
		if (!repo) return;
		issueStates.update((current) => {
			current.set(`${repo.id}:${issue.number}`, columnId);
			return new Map(current);
		});
		backend.setIssueColumn(repo.id, issue.number, columnId).catch(() => {});
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
