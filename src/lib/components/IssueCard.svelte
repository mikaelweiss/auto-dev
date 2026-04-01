<script lang="ts">
  import { Square, Play, AlertCircle, Loader } from "lucide-svelte";
  import type { Issue } from "../types";
  import { timeAgo } from "../utils";
  import { setSelectedIssue, stopSession, resumeSession } from "../store.svelte";

  let { issue }: { issue: Issue } = $props();

  function stageLabel(stage: Issue["stage"]): string {
    if (!stage) return "";
    const labels: Record<string, string> = {
      spec: "Spec",
      implement: "Implement",
      review: "Review",
      "ci-fix": "CI Fix",
      "merge-conflict": "Merge Conflict",
    };
    return labels[stage] ?? stage;
  }

  function handleDragStart(e: DragEvent) {
    if (issue.sessionState === "in-progress" || issue.sessionState === "initializing") {
      e.preventDefault();
      return;
    }
    e.dataTransfer?.setData("text/plain", String(issue.id));
  }

  function handleStop(e: MouseEvent) {
    e.stopPropagation();
    stopSession(issue.id);
  }

  function handleResume(e: MouseEvent) {
    e.stopPropagation();
    resumeSession(issue.id);
  }

  let isActive = $derived(
    issue.sessionState === "in-progress" || issue.sessionState === "initializing"
  );
  let isDraggable = $derived(!isActive);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="group w-full cursor-pointer rounded-lg border border-border bg-card p-3 text-left transition-all hover:border-muted-foreground/30"
  class:cursor-grab={isDraggable}
  class:cursor-not-allowed={!isDraggable}
  draggable={isDraggable}
  ondragstart={handleDragStart}
  onclick={() => setSelectedIssue(issue)}
  onkeydown={(e) => { if (e.key === 'Enter') setSelectedIssue(issue); }}
  role="button"
  tabindex={0}
>
  <!-- Title row -->
  <div class="flex items-start justify-between gap-2">
    <h3 class="text-sm font-medium leading-snug text-foreground">{issue.title}</h3>
    {#if issue.hasError}
      <span class="mt-0.5 shrink-0 rounded-full bg-red-500/20 p-1">
        <AlertCircle size={12} class="text-red-400" />
      </span>
    {/if}
  </div>

  <!-- Meta row -->
  <div class="mt-2 flex items-center justify-between">
    <div class="flex items-center gap-2">
      <span class="text-xs text-muted-foreground">#{issue.number}</span>

      {#if issue.stage && issue.column !== "done"}
        <span class="flex items-center gap-1 rounded-full bg-secondary px-1.5 py-0.5 text-[10px] font-medium text-secondary-foreground">
          {#if issue.sessionState === "in-progress"}
            <Loader size={10} class="animate-spin" />
          {/if}
          {#if issue.sessionState === "initializing"}
            <Loader size={10} class="animate-spin text-yellow-400" />
          {/if}
          {stageLabel(issue.stage)}
        </span>
      {/if}

      {#if issue.column === "blocked"}
        <span class="pulse-dot h-2 w-2 rounded-full bg-orange-400"></span>
      {/if}
    </div>

    <div class="flex items-center gap-2">
      {#if isActive}
        <button
          onclick={handleStop}
          class="rounded p-0.5 text-muted-foreground opacity-0 transition-all hover:bg-destructive/20 hover:text-red-400 group-hover:opacity-100"
          title="Stop session"
        >
          <Square size={12} />
        </button>
      {/if}

      {#if issue.sessionState === "canceled"}
        <button
          onclick={handleResume}
          class="rounded p-0.5 text-muted-foreground opacity-0 transition-all hover:bg-accent hover:text-green-400 group-hover:opacity-100"
          title="Resume session"
        >
          <Play size={12} />
        </button>
      {/if}

      <span class="text-[10px] text-muted-foreground">{timeAgo(issue.updatedAt)}</span>

      {#if issue.assignee}
        <img
          src={issue.assignee.avatarUrl}
          alt={issue.assignee.login}
          class="h-5 w-5 rounded-full"
        />
      {/if}
    </div>
  </div>
</div>
