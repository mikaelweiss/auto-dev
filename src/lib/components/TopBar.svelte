<script lang="ts">
  import { Settings, ChevronDown, Plus } from "lucide-svelte";
  import {
    getSelectedRepo,
    setSelectedRepo,
    getRepos,
    getCurrentUser,
    setShowNewIssueDialog,
    setShowSettingsDialog,
  } from "../store.svelte";

  let repoDropdownOpen = $state(false);

  function toggleRepoDropdown() {
    repoDropdownOpen = !repoDropdownOpen;
  }

  function selectRepo(repo: ReturnType<typeof getRepos>[0]) {
    setSelectedRepo(repo);
    repoDropdownOpen = false;
  }

  function handleClickOutside(e: MouseEvent) {
    if (repoDropdownOpen) {
      const target = e.target as HTMLElement;
      if (!target.closest(".repo-dropdown")) {
        repoDropdownOpen = false;
      }
    }
  }
</script>

<svelte:window onclick={handleClickOutside} />

<header class="flex h-12 shrink-0 items-center justify-between border-b border-border bg-background px-4">
  <div class="flex items-center gap-4">
    <span class="text-sm font-semibold tracking-tight text-foreground">AutoDev</span>

    <div class="repo-dropdown relative">
      <button
        onclick={toggleRepoDropdown}
        class="flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
      >
        {getSelectedRepo().fullName}
        <ChevronDown size={12} />
      </button>

      {#if repoDropdownOpen}
        <div class="absolute left-0 top-full z-50 mt-1 min-w-[200px] rounded-md border border-border bg-card p-1 shadow-lg">
          {#each getRepos() as repo}
            <button
              onclick={() => selectRepo(repo)}
              class="flex w-full items-center rounded-sm px-2 py-1.5 text-xs text-foreground transition-colors hover:bg-accent"
              class:bg-accent={getSelectedRepo().fullName === repo.fullName}
            >
              {repo.fullName}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="flex items-center gap-2">
    <button
      onclick={() => setShowNewIssueDialog(true)}
      class="flex items-center gap-1.5 rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
    >
      <Plus size={12} />
      New Issue
    </button>

    <button
      onclick={() => setShowSettingsDialog(true)}
      class="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
    >
      <Settings size={16} />
    </button>

    <img
      src={getCurrentUser().avatarUrl}
      alt={getCurrentUser().login}
      class="h-7 w-7 rounded-full"
    />
  </div>
</header>
