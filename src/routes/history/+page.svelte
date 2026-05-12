<script lang="ts">
  import { onMount } from "svelte";
  import { history } from "$lib/history-store.svelte";
  import { api, type Recording } from "$lib/api";
  import HistoryRow from "$lib/HistoryRow.svelte";
  import { openPath } from "@tauri-apps/plugin-opener";

  let filter = $state<"all" | "light" | "advanced" | "drafting" | "error">("all");
  let search = $state("");

  onMount(() => {
    history.refresh();
    history.subscribe();
  });

  let filtered = $derived.by(() => {
    let list = history.list;
    if (filter !== "all") {
      list = list.filter((r) => (filter === "error" ? r.status === "error" : r.mode === filter));
    }
    const q = search.trim().toLowerCase();
    if (q) {
      list = list.filter((r) => {
        const t = (r.cleaned_text || r.transcript || "").toLowerCase();
        return t.includes(q);
      });
    }
    return list;
  });

  // Group filtered list by date bucket for visual dividers in the timeline.
  type Group = { label: string; items: Recording[] };
  let grouped = $derived.by<Group[]>(() => {
    const now = new Date();
    const today = now.toDateString();
    const yesterday = new Date(now); yesterday.setDate(now.getDate() - 1);
    const ystr = yesterday.toDateString();
    const weekAgo = new Date(now); weekAgo.setDate(now.getDate() - 7);

    const buckets: Record<string, Recording[]> = {};
    const order: string[] = [];
    function bucketLabel(r: Recording): string {
      const d = new Date(r.created_at);
      const ds = d.toDateString();
      if (ds === today) return "Today";
      if (ds === ystr) return "Yesterday";
      if (d.getTime() > weekAgo.getTime()) return "Earlier this week";
      if (d.getFullYear() === now.getFullYear()) {
        return d.toLocaleDateString([], { month: "long" });
      }
      return d.toLocaleDateString([], { year: "numeric", month: "long" });
    }
    for (const r of filtered) {
      const label = bucketLabel(r);
      if (!buckets[label]) {
        buckets[label] = [];
        order.push(label);
      }
      buckets[label].push(r);
    }
    return order.map((label) => ({ label, items: buckets[label] }));
  });

  async function clearAll() {
    const ok = confirm(
      "Delete ALL recordings?\n\n" +
      "This permanently removes:\n" +
      "  • All transcripts and cleaned/drafted text\n" +
      "  • All audio files (the .wav recordings)\n" +
      "  • All history rows\n\n" +
      "This cannot be undone. Are you sure?"
    );
    if (!ok) return;
    const removed = await api.clearAllHistory();
    await history.refresh();
    alert(`Deleted ${removed} recording${removed === 1 ? "" : "s"}.`);
  }

  async function openRecordingsFolder() {
    try {
      const paths = await api.appPaths();
      await openPath(paths.audio_dir);
    } catch (e) {
      alert(`Could not open folder: ${e}`);
    }
  }
</script>

<section class="history">
  <header class="history-head">
    <div class="title-row">
      <h1>History</h1>
      <span class="count">{filtered.length} of {history.list.length}</span>
    </div>

    <div class="search-row">
      <svg class="search-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.8" />
        <path d="M 10.5 10.5 L 14 14" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
      </svg>
      <input
        type="search"
        class="search"
        placeholder="Search transcripts…"
        bind:value={search}
      />
      {#if search}
        <button class="search-clear" onclick={() => (search = "")} aria-label="Clear search">×</button>
      {/if}
    </div>

    <div class="controls">
      <div class="filter-pills">
        <button class="pill" class:active={filter === "all"} onclick={() => (filter = "all")}>All</button>
        <button class="pill" class:active={filter === "light"} onclick={() => (filter = "light")}>Light</button>
        <button class="pill" class:active={filter === "advanced"} onclick={() => (filter = "advanced")}>Advanced</button>
        <button class="pill" class:active={filter === "drafting"} onclick={() => (filter = "drafting")}>Drafting</button>
        <button class="pill error-pill" class:active={filter === "error"} onclick={() => (filter = "error")}>Errors</button>
      </div>

      <div class="controls-right">
        <button class="icon-btn" onclick={openRecordingsFolder} title="Open recordings folder in file manager">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M 2 4 L 2 12 A 1 1 0 0 0 3 13 L 13 13 A 1 1 0 0 0 14 12 L 14 5 A 1 1 0 0 0 13 4 L 8 4 L 6.5 2.5 A 1 1 0 0 0 5.8 2.2 L 3 2.2 A 1 1 0 0 0 2 3.2 Z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
          </svg>
          Folder
        </button>
        <button class="icon-btn" onclick={() => history.refresh()} title="Reload the list from disk — does not re-transcribe anything">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
        </button>
        <button class="icon-btn danger" onclick={clearAll} title="Delete all recordings">Clear all</button>
      </div>
    </div>
  </header>

  {#if history.loading && history.list.length === 0}
    <p class="muted">Loading…</p>
  {:else if history.list.length === 0}
    <div class="empty">
      <p class="empty-title">No recordings yet</p>
      <p class="empty-body">Hold your hotkey anywhere to dictate. The recording will appear here.</p>
    </div>
  {:else if filtered.length === 0}
    <div class="empty">
      <p class="empty-title">Nothing matches</p>
      <p class="empty-body">Try a different filter or search term.</p>
    </div>
  {:else}
    <div class="rows">
      {#each grouped as group (group.label)}
        <div class="date-divider">
          <span class="date-label">{group.label}</span>
          <span class="date-count">{group.items.length}</span>
          <span class="date-line"></span>
        </div>
        {#each group.items as rec (rec.id)}
          <HistoryRow {rec} />
        {/each}
      {/each}
    </div>
  {/if}
</section>

<style>
  .history {
    padding: 0;
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .history-head {
    padding: 20px 28px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: var(--bg-elev);
    flex-shrink: 0;
    box-shadow: 0 1px 0 var(--border-subtle);
  }

  .title-row {
    margin-bottom: 2px;
  }

  .title-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }

  h1 {
    font-size: 22px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
  }

  .count {
    font-size: 12px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .search-row {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 11px;
    color: var(--text-secondary);
    pointer-events: none;
  }

  .search {
    flex: 1;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 30px 8px 32px;
    font-size: 13px;
    background: var(--bg-card);
    color: var(--text-primary);
    outline: none;
    transition: border-color 120ms ease, box-shadow 120ms ease;
  }

  .search::placeholder {
    color: var(--text-secondary);
  }

  .search:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-fade);
  }

  .search-clear {
    position: absolute;
    right: 6px;
    width: 22px;
    height: 22px;
    border: none;
    background: var(--bg-subtle);
    border-radius: 50%;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  .search-clear:hover {
    background: var(--text-secondary);
    color: var(--bg-card);
  }

  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .filter-pills {
    display: flex;
    gap: 4px;
    background: var(--bg-subtle);
    padding: 4px;
    border-radius: 9px;
  }

  .pill {
    background: transparent;
    border: none;
    padding: 5px 12px;
    font-size: 12px;
    color: var(--text-secondary);
    border-radius: 6px;
    cursor: pointer;
    transition: all 100ms ease;
    font-weight: 500;
  }

  .pill:hover {
    color: var(--text-primary);
  }

  .pill.active {
    background: var(--bg-card);
    color: var(--text-primary);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.10);
  }

  .pill.error-pill.active {
    color: var(--danger);
  }

  .controls-right {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .icon-btn {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 7px 12px;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-primary);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    transition: all 120ms ease;
  }

  .icon-btn:hover {
    background: var(--bg-subtle);
    border-color: var(--text-secondary);
  }

  .icon-btn.danger {
    color: var(--danger);
  }

  .icon-btn.danger:hover {
    background: var(--danger-fade);
    border-color: var(--danger);
  }

  /* Date dividers */
  .date-divider {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 24px 8px;
    background: var(--bg-surface);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .date-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  .date-count {
    font-size: 10px;
    color: var(--text-secondary);
    background: var(--bg-subtle);
    padding: 1px 7px;
    border-radius: 9999px;
    flex-shrink: 0;
  }

  .date-line {
    flex: 1;
    height: 1px;
    background: var(--border-subtle);
  }

  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 0;
  }

  .muted {
    color: #86868b;
    font-size: 14px;
    padding: 20px 24px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--text-secondary);
    text-align: center;
    padding: 40px;
  }

  .empty-title {
    font-size: 16px;
    color: var(--text-primary);
    margin: 0 0 6px;
    font-weight: 500;
  }

  .empty-body {
    font-size: 13px;
    margin: 0;
  }
</style>
