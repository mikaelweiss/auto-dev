<script lang="ts">
  import { X } from "lucide-svelte";
  import { setShowNewIssueDialog, addIssue } from "../store.svelte";

  let title = $state("");
  let body = $state("");

  function close() {
    setShowNewIssueDialog(false);
  }

  function handleBackdropClick(e: MouseEvent) {
    if ((e.target as HTMLElement).classList.contains("modal-backdrop")) {
      close();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  function create() {
    if (!title.trim()) return;
    addIssue(title, body, false);
    close();
  }

  function createAndStart() {
    if (!title.trim()) return;
    addIssue(title, body, true);
    close();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="modal-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-8"
  onclick={handleBackdropClick}
  onkeydown={handleKeydown}
  role="presentation"
>
  <div class="w-full max-w-[500px] rounded-xl border border-border bg-card shadow-2xl">
    <!-- Header -->
    <div class="flex items-center justify-between border-b border-border px-5 py-3">
      <h2 class="text-sm font-semibold text-foreground">New Issue</h2>
      <button
        onclick={close}
        class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      >
        <X size={16} />
      </button>
    </div>

    <!-- Body -->
    <div class="space-y-4 p-5">
      <div>
        <label for="title" class="mb-1.5 block text-xs font-medium text-muted-foreground">
          Title
        </label>
        <input
          id="title"
          bind:value={title}
          placeholder="Issue title..."
          class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
        />
      </div>

      <div>
        <label for="body" class="mb-1.5 block text-xs font-medium text-muted-foreground">
          Description
        </label>
        <textarea
          id="body"
          bind:value={body}
          placeholder="Describe the issue..."
          rows={6}
          class="w-full resize-none rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
        ></textarea>
      </div>
    </div>

    <!-- Footer -->
    <div class="flex justify-end gap-2 border-t border-border px-5 py-3">
      <button
        onclick={close}
        class="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      >
        Cancel
      </button>
      <button
        onclick={create}
        class="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent"
      >
        Create
      </button>
      <button
        onclick={createAndStart}
        class="rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
      >
        Create & Start
      </button>
    </div>
  </div>
</div>
