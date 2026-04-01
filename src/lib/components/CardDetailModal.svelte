<script lang="ts">
  import {
    X,
    ExternalLink,
    FlaskConical,
    GitMerge,
    Send,
    Clock,
    Loader,
    FileCode,
    Terminal,
  } from "lucide-svelte";
  import type { Issue, ChatMessage } from "../types";
  import { setSelectedIssue } from "../store.svelte";
  import { fakeChatMessages } from "../fake-data";
  import { timeAgo } from "../utils";

  let { issue }: { issue: Issue } = $props();

  let chatInput = $state("");
  let messages = $state<ChatMessage[]>([...fakeChatMessages]);

  function close() {
    setSelectedIssue(null);
  }

  function handleBackdropClick(e: MouseEvent) {
    if ((e.target as HTMLElement).classList.contains("modal-backdrop")) {
      close();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  function sendMessage() {
    if (!chatInput.trim()) return;
    messages.push({
      id: Date.now(),
      role: "user",
      content: chatInput,
      timestamp: new Date(),
    });
    chatInput = "";
  }

  function handleInputKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  function stageLabel(stage: Issue["stage"]): string {
    if (!stage) return "None";
    const labels: Record<string, string> = {
      spec: "Spec",
      implement: "Implement",
      review: "Review",
      "ci-fix": "CI Fix",
      "merge-conflict": "Merge Conflict",
    };
    return labels[stage] ?? stage;
  }

  function sessionStateLabel(state: Issue["sessionState"]): string {
    if (!state) return "Idle";
    const labels: Record<string, string> = {
      initializing: "Initializing",
      "in-progress": "Running",
      canceled: "Canceled",
    };
    return labels[state] ?? state;
  }

  function roleIcon(role: ChatMessage["role"]) {
    return role;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="modal-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-8"
  onclick={handleBackdropClick}
  onkeydown={handleKeydown}
  role="presentation"
>
  <div class="flex h-full max-h-[700px] w-full max-w-[800px] flex-col overflow-hidden rounded-xl border border-border bg-card shadow-2xl">
    <!-- Header -->
    <div class="flex shrink-0 items-center justify-between border-b border-border px-5 py-3">
      <div class="flex items-center gap-3">
        <h2 class="text-sm font-semibold text-foreground">{issue.title}</h2>
        <span class="text-xs text-muted-foreground">#{issue.number}</span>
      </div>
      <div class="flex items-center gap-2">
        {#if issue.prUrl}
          <a
            href={issue.prUrl}
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <ExternalLink size={12} />
            GitHub
          </a>
        {/if}
        <button
          onclick={close}
          class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <X size={16} />
        </button>
      </div>
    </div>

    <!-- Body: two-column layout -->
    <div class="flex flex-1 overflow-hidden">
      <!-- Left: Issue info + session info -->
      <div class="flex w-[260px] shrink-0 flex-col border-r border-border">
        <!-- Issue body -->
        <div class="flex-1 overflow-y-auto p-4">
          <h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Description
          </h3>
          <p class="whitespace-pre-wrap text-xs leading-relaxed text-foreground/80">
            {issue.body}
          </p>
        </div>

        <!-- Session info -->
        <div class="border-t border-border p-4">
          <h3 class="mb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Session
          </h3>
          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <span class="text-xs text-muted-foreground">Stage</span>
              <span class="text-xs font-medium text-foreground">{stageLabel(issue.stage)}</span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-xs text-muted-foreground">Status</span>
              <span class="flex items-center gap-1 text-xs font-medium text-foreground">
                {#if issue.sessionState === "in-progress"}
                  <Loader size={10} class="animate-spin text-green-400" />
                {/if}
                {sessionStateLabel(issue.sessionState)}
              </span>
            </div>
            <div class="flex items-center justify-between">
              <span class="text-xs text-muted-foreground">Updated</span>
              <span class="flex items-center gap-1 text-xs text-foreground">
                <Clock size={10} />
                {timeAgo(issue.updatedAt)}
              </span>
            </div>
            {#if issue.assignee}
              <div class="flex items-center justify-between">
                <span class="text-xs text-muted-foreground">Assignee</span>
                <div class="flex items-center gap-1.5">
                  <img src={issue.assignee.avatarUrl} alt="" class="h-4 w-4 rounded-full" />
                  <span class="text-xs text-foreground">{issue.assignee.login}</span>
                </div>
              </div>
            {/if}
          </div>

          {#if issue.column === "review"}
            <div class="mt-4 flex gap-2">
              <button class="flex flex-1 items-center justify-center gap-1.5 rounded-md border border-border py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent">
                <FlaskConical size={12} />
                Test
              </button>
              <button class="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-green-600 py-1.5 text-xs font-medium text-white transition-colors hover:bg-green-700">
                <GitMerge size={12} />
                Merge
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Right: Chat -->
      <div class="flex flex-1 flex-col">
        <!-- Messages -->
        <div class="flex-1 overflow-y-auto p-4">
          <div class="space-y-4">
            {#each messages as msg (msg.id)}
              {#if msg.role === "tool"}
                <div class="rounded-md border border-border bg-background p-3">
                  <div class="mb-1.5 flex items-center gap-1.5 text-[10px] font-medium text-muted-foreground">
                    <Terminal size={10} />
                    {msg.toolName ?? "Tool"}
                  </div>
                  <pre class="whitespace-pre-wrap text-xs text-foreground/70">{msg.content}</pre>
                </div>
              {:else if msg.role === "assistant"}
                <div>
                  <div class="mb-1 flex items-center gap-1.5">
                    <div class="flex h-5 w-5 items-center justify-center rounded-full bg-primary text-[10px] font-bold text-primary-foreground">
                      C
                    </div>
                    <span class="text-[10px] text-muted-foreground">{timeAgo(msg.timestamp)}</span>
                  </div>
                  <div class="ml-6 text-xs leading-relaxed text-foreground/90 whitespace-pre-wrap">{msg.content}</div>
                </div>
              {:else}
                <div class="flex justify-end">
                  <div class="max-w-[80%]">
                    <div class="mb-1 flex items-center justify-end gap-1.5">
                      <span class="text-[10px] text-muted-foreground">{timeAgo(msg.timestamp)}</span>
                    </div>
                    <div class="rounded-lg bg-primary px-3 py-2 text-xs text-primary-foreground">
                      {msg.content}
                    </div>
                  </div>
                </div>
              {/if}
            {/each}
          </div>
        </div>

        <!-- Input -->
        <div class="shrink-0 border-t border-border p-3">
          <div class="flex items-end gap-2">
            <textarea
              bind:value={chatInput}
              onkeydown={handleInputKeydown}
              placeholder="Send a message to Claude..."
              rows={1}
              class="flex-1 resize-none rounded-md border border-border bg-background px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            ></textarea>
            <button
              onclick={sendMessage}
              class="rounded-md bg-primary p-2 text-primary-foreground transition-colors hover:bg-primary/90"
            >
              <Send size={14} />
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
