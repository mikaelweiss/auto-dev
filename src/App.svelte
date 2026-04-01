<script lang="ts">
  import TopBar from "./lib/components/TopBar.svelte";
  import KanbanBoard from "./lib/components/KanbanBoard.svelte";
  import CardDetailModal from "./lib/components/CardDetailModal.svelte";
  import NewIssueDialog from "./lib/components/NewIssueDialog.svelte";
  import SettingsDialog from "./lib/components/SettingsDialog.svelte";
  import {
    getSelectedIssue,
    getShowNewIssueDialog,
    getShowSettingsDialog,
  } from "./lib/store.svelte";
</script>

<div class="flex h-screen flex-col">
  <TopBar />
  <KanbanBoard />
</div>

{#if getSelectedIssue()}
  <CardDetailModal issue={getSelectedIssue()!} />
{/if}

{#if getShowNewIssueDialog()}
  <NewIssueDialog />
{/if}

{#if getShowSettingsDialog()}
  <SettingsDialog />
{/if}
