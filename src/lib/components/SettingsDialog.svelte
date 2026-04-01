<script lang="ts">
  import { X, RotateCcw } from "lucide-svelte";
  import {
    setShowSettingsDialog,
    getAppSettings,
    setAppSettings,
    getRepoSettings,
    setRepoSettings,
    getPrompts,
    setPrompt,
    getSelectedRepo,
  } from "../store.svelte";
  import { defaultPrompts } from "../fake-data";

  type Tab = "app" | "repo" | "prompts";
  let activeTab = $state<Tab>("app");

  const promptKeys = [
    { key: "spec", label: "Spec" },
    { key: "implement", label: "Implement" },
    { key: "review", label: "Review" },
    { key: "ci-fix", label: "CI Fix" },
    { key: "merge-conflict", label: "Merge Conflict" },
  ];

  let activePromptKey = $state("spec");

  function close() {
    setShowSettingsDialog(false);
  }

  function handleBackdropClick(e: MouseEvent) {
    if ((e.target as HTMLElement).classList.contains("modal-backdrop")) {
      close();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  function restoreDefaultPrompt() {
    setPrompt(activePromptKey, defaultPrompts[activePromptKey]);
  }

  function toggleSleepPrevention() {
    const s = getAppSettings();
    setAppSettings({ ...s, sleepPrevention: !s.sleepPrevention });
  }

  function toggleNotifications() {
    const s = getAppSettings();
    setAppSettings({ ...s, notifications: !s.notifications });
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="modal-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-8"
  onclick={handleBackdropClick}
  onkeydown={handleKeydown}
  role="presentation"
>
  <div class="flex h-full max-h-[550px] w-full max-w-[650px] flex-col overflow-hidden rounded-xl border border-border bg-card shadow-2xl">
    <!-- Header -->
    <div class="flex shrink-0 items-center justify-between border-b border-border px-5 py-3">
      <h2 class="text-sm font-semibold text-foreground">Settings</h2>
      <button
        onclick={close}
        class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      >
        <X size={16} />
      </button>
    </div>

    <!-- Tabs -->
    <div class="flex shrink-0 border-b border-border px-5">
      {#each [
        { id: "app" as Tab, label: "App" },
        { id: "repo" as Tab, label: `Repo (${getSelectedRepo().name})` },
        { id: "prompts" as Tab, label: "Agent Prompts" },
      ] as tab}
        <button
          onclick={() => (activeTab = tab.id)}
          class="border-b-2 px-3 py-2 text-xs font-medium transition-colors"
          class:border-foreground={activeTab === tab.id}
          class:text-foreground={activeTab === tab.id}
          class:border-transparent={activeTab !== tab.id}
          class:text-muted-foreground={activeTab !== tab.id}
          class:hover:text-foreground={activeTab !== tab.id}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-5">
      {#if activeTab === "app"}
        <div class="space-y-4">
          <!-- Sleep prevention -->
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm font-medium text-foreground">Sleep Prevention</div>
              <div class="text-xs text-muted-foreground">
                Prevents macOS idle sleep while sessions are running
              </div>
            </div>
            <button
              aria-label="Toggle sleep prevention"
              onclick={toggleSleepPrevention}
              class="relative h-5 w-9 rounded-full transition-colors"
              class:bg-primary={getAppSettings().sleepPrevention}
              class:bg-muted={!getAppSettings().sleepPrevention}
            >
              <span
                class="absolute top-0.5 h-4 w-4 rounded-full bg-background shadow transition-transform"
                class:translate-x-4={getAppSettings().sleepPrevention}
                class:translate-x-0.5={!getAppSettings().sleepPrevention}
              ></span>
            </button>
          </div>

          <!-- Notifications -->
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm font-medium text-foreground">Notifications</div>
              <div class="text-xs text-muted-foreground">
                macOS notifications for Blocked and Ready for Review
              </div>
            </div>
            <button
              aria-label="Toggle notifications"
              onclick={toggleNotifications}
              class="relative h-5 w-9 rounded-full transition-colors"
              class:bg-primary={getAppSettings().notifications}
              class:bg-muted={!getAppSettings().notifications}
            >
              <span
                class="absolute top-0.5 h-4 w-4 rounded-full bg-background shadow transition-transform"
                class:translate-x-4={getAppSettings().notifications}
                class:translate-x-0.5={!getAppSettings().notifications}
              ></span>
            </button>
          </div>

          <!-- Poll interval -->
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm font-medium text-foreground">Poll Interval</div>
              <div class="text-xs text-muted-foreground">
                How often to poll GitHub (seconds)
              </div>
            </div>
            <input
              type="number"
              value={getAppSettings().pollInterval}
              oninput={(e) => {
                const v = Number((e.target as HTMLInputElement).value);
                if (v > 0) setAppSettings({ ...getAppSettings(), pollInterval: v });
              }}
              min="5"
              max="300"
              class="w-16 rounded-md border border-border bg-background px-2 py-1 text-center text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            />
          </div>
        </div>
      {:else if activeTab === "repo"}
        <div class="space-y-4">
          {#each [
            {
              key: "setupScript",
              label: "Setup Script",
              desc: "Run after worktree creation (e.g., bun install)",
              type: "text",
            },
            {
              key: "runScript",
              label: "Run Script",
              desc: "Run when clicking Test (e.g., bun run dev)",
              type: "text",
            },
            {
              key: "baseBranch",
              label: "Base Branch",
              desc: "Branch to create worktrees from",
              type: "text",
            },
            {
              key: "branchPrefix",
              label: "Branch Prefix",
              desc: "Prefix for worktree branches",
              type: "text",
            },
            {
              key: "worktreeDir",
              label: "Worktree Directory",
              desc: "Directory within repo for worktrees",
              type: "text",
            },
          ] as field}
            <div>
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="mb-1 block text-sm font-medium text-foreground">
                {field.label}
              </label>
              <div class="mb-1.5 text-xs text-muted-foreground">{field.desc}</div>
              <input
                type="text"
                value={getRepoSettings()[field.key as keyof ReturnType<typeof getRepoSettings>]}
                oninput={(e) => {
                  const val = (e.target as HTMLInputElement).value;
                  setRepoSettings({
                    ...getRepoSettings(),
                    [field.key]: val,
                  });
                }}
                class="w-full rounded-md border border-border bg-background px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
          {/each}
        </div>
      {:else if activeTab === "prompts"}
        <div class="flex h-full gap-4">
          <!-- Prompt selector -->
          <div class="w-[140px] shrink-0 space-y-1">
            {#each promptKeys as pk}
              <button
                onclick={() => (activePromptKey = pk.key)}
                class="w-full rounded-md px-2.5 py-1.5 text-left text-xs transition-colors"
                class:bg-accent={activePromptKey === pk.key}
                class:text-foreground={activePromptKey === pk.key}
                class:text-muted-foreground={activePromptKey !== pk.key}
                class:hover:bg-accent={activePromptKey !== pk.key}
              >
                {pk.label}
              </button>
            {/each}
          </div>

          <!-- Prompt editor -->
          <div class="flex flex-1 flex-col">
            <div class="mb-2 flex items-center justify-between">
              <span class="text-xs font-medium text-foreground">
                {promptKeys.find((p) => p.key === activePromptKey)?.label} Prompt
              </span>
              <button
                onclick={restoreDefaultPrompt}
                class="flex items-center gap-1 rounded-md px-2 py-1 text-[10px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                <RotateCcw size={10} />
                Restore Default
              </button>
            </div>
            <textarea
              value={getPrompts()[activePromptKey]}
              oninput={(e) => setPrompt(activePromptKey, (e.target as HTMLTextAreaElement).value)}
              class="flex-1 resize-none rounded-md border border-border bg-background p-3 font-mono text-xs leading-relaxed text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              rows={12}
            ></textarea>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
