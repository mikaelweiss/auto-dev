<script lang="ts">
  import type { Column } from "../types";
  import { getIssuesByColumn, moveIssue } from "../store.svelte";
  import IssueCard from "./IssueCard.svelte";

  let { column, title }: { column: Column; title: string } = $props();

  let dragOver = $state(false);

  const columnColors: Record<Column, string> = {
    backlog: "text-muted-foreground",
    claimed: "text-blue-400",
    "in-progress": "text-yellow-400",
    blocked: "text-orange-400",
    review: "text-purple-400",
    done: "text-green-400",
  };

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    dragOver = true;
  }

  function handleDragLeave() {
    dragOver = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    const issueId = Number(e.dataTransfer?.getData("text/plain"));
    if (issueId) {
      moveIssue(issueId, column);
    }
  }

  let issues = $derived(getIssuesByColumn(column));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  role="region"
  aria-label="{title} column"
  class="flex h-full w-[260px] shrink-0 flex-col rounded-lg"
  class:drop-target={dragOver}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  <!-- Column header -->
  <div class="flex items-center gap-2 px-2 pb-3">
    <span class="text-xs font-semibold uppercase tracking-wider {columnColors[column]}">
      {title}
    </span>
    <span class="rounded-full bg-secondary px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
      {issues.length}
    </span>
  </div>

  <!-- Cards -->
  <div class="column-scroll flex flex-1 flex-col gap-2 overflow-y-auto px-1 pb-2">
    {#each issues as issue (issue.id)}
      <IssueCard {issue} />
    {/each}

    {#if issues.length === 0}
      <div class="flex items-center justify-center rounded-lg border border-dashed border-border py-8 text-xs text-muted-foreground">
        No issues
      </div>
    {/if}
  </div>
</div>
