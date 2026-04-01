<script lang="ts">
  import KanbanColumn from "./KanbanColumn.svelte";
</script>

<div class="flex flex-1 gap-4 overflow-x-auto p-4">
  <KanbanColumn column="backlog" title="Backlog" />
  <KanbanColumn column="claimed" title="Claimed" />
  <KanbanColumn column="in-progress" title="In Progress" />
  <KanbanColumn column="blocked" title="Blocked" />
  <KanbanColumn column="review" title="Ready for Review" />
  <KanbanColumn column="done" title="Done" />
</div>
