<script lang="ts">
  import { onMount } from "svelte";
  import { history } from "$lib/history-store.svelte";
  import HistoryRow from "$lib/HistoryRow.svelte";

  onMount(() => {
    history.refresh();
    history.subscribe();
  });
</script>

<main class="flex flex-col min-h-screen p-5 gap-3 max-w-3xl mx-auto">
  <header class="flex items-center justify-between">
    <a href="/" class="text-sm text-blue-600 hover:underline">← Back</a>
    <h1 class="text-base font-semibold">History</h1>
    <button
      class="text-xs px-2 py-1 rounded border hover:bg-gray-50"
      onclick={() => history.refresh()}
    >
      Refresh
    </button>
  </header>

  {#if history.loading && history.list.length === 0}
    <p class="text-sm text-gray-500">Loading…</p>
  {:else if history.list.length === 0}
    <p class="text-sm text-gray-500">No recordings yet. Press your hotkey to create one.</p>
  {:else}
    <div class="overflow-auto rounded border bg-white">
      <table class="w-full text-left">
        <thead class="bg-gray-50 sticky top-0">
          <tr class="text-xs uppercase text-gray-500">
            <th class="px-2 py-1.5 font-medium">When</th>
            <th class="px-2 py-1.5 font-medium">Length</th>
            <th class="px-2 py-1.5 font-medium">Mode</th>
            <th class="px-2 py-1.5 font-medium">Transcript</th>
            <th class="px-2 py-1.5 font-medium"></th>
          </tr>
        </thead>
        <tbody>
          {#each history.list as rec (rec.id)}
            <HistoryRow {rec} />
          {/each}
        </tbody>
      </table>
    </div>
    <p class="text-xs text-gray-500">{history.list.length} recordings.</p>
  {/if}
</main>
