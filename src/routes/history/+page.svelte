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
    <!-- Loading state: sleeping-fox placeholder SVG + caption. Replaced
         later with the user-provided "curled-up sleeping fox" PNG. -->
    <div class="empty">
      <svg class="empty-fox sleeping" viewBox="0 0 200 120" width="160" height="100" aria-hidden="true">
        <!-- shadow -->
        <ellipse cx="100" cy="105" rx="60" ry="6" fill="rgba(120, 80, 30, 0.18)" />
        <!-- body (oval) -->
        <ellipse cx="100" cy="78" rx="56" ry="22" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" />
        <!-- cream belly -->
        <ellipse cx="100" cy="86" rx="40" ry="12" fill="#fde2c4" />
        <!-- curled tail -->
        <path d="M 152 78 C 168 70, 170 56, 156 50 C 148 56, 144 70, 152 78 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" />
        <path d="M 156 50 Q 160 54, 158 60" fill="none" stroke="#fde2c4" stroke-width="2.5" stroke-linecap="round" />
        <!-- head resting on body -->
        <ellipse cx="50" cy="72" rx="22" ry="18" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" />
        <!-- ears -->
        <path d="M 34 60 L 36 48 L 44 56 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" stroke-linejoin="round"/>
        <path d="M 60 56 L 62 44 L 70 56 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" stroke-linejoin="round"/>
        <!-- closed eyes (sleeping) -->
        <path d="M 42 70 Q 46 73, 50 70" fill="none" stroke="#2b2218" stroke-width="1.8" stroke-linecap="round" />
        <path d="M 56 70 Q 60 73, 64 70" fill="none" stroke="#2b2218" stroke-width="1.8" stroke-linecap="round" />
        <!-- nose -->
        <ellipse cx="36" cy="76" rx="2" ry="1.4" fill="#2b2218" />
        <!-- ZZZ -->
        <text x="32" y="36" font-size="14" font-weight="600" fill="#7d6a55" font-family="Inter, sans-serif">z</text>
        <text x="22" y="28" font-size="11" font-weight="600" fill="#b3a08a" font-family="Inter, sans-serif">z</text>
      </svg>
      <p class="empty-title">Loading transcripts…</p>
    </div>
  {:else if history.list.length === 0}
    <!-- First-run empty state: sitting fox placeholder + helpful copy.
         Replaced later with the user-provided "fox in flowers" PNG. -->
    <div class="empty">
      <svg class="empty-fox sitting" viewBox="0 0 160 140" width="160" height="140" aria-hidden="true">
        <!-- ground shadow -->
        <ellipse cx="80" cy="125" rx="48" ry="6" fill="rgba(120, 80, 30, 0.18)" />
        <!-- grass + flower clumps -->
        <path d="M 18 124 Q 14 116, 22 110" fill="none" stroke="#6cb16d" stroke-width="2" stroke-linecap="round" />
        <path d="M 26 126 Q 30 118, 26 112" fill="none" stroke="#6cb16d" stroke-width="2" stroke-linecap="round" />
        <path d="M 138 126 Q 134 118, 138 112" fill="none" stroke="#6cb16d" stroke-width="2" stroke-linecap="round" />
        <path d="M 132 124 Q 136 116, 130 110" fill="none" stroke="#6cb16d" stroke-width="2" stroke-linecap="round" />
        <circle cx="14" cy="120" r="4" fill="#d479ad" /><circle cx="14" cy="120" r="1.6" fill="#fde2c4" />
        <circle cx="146" cy="118" r="4" fill="#8d5dce" /><circle cx="146" cy="118" r="1.6" fill="#fde2c4" />
        <!-- legs / sitting body -->
        <path d="M 42 110 C 42 84, 118 84, 118 110 L 118 122 C 118 128, 42 128, 42 122 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" stroke-linejoin="round" />
        <!-- chest fluff -->
        <ellipse cx="80" cy="106" rx="22" ry="12" fill="#fde2c4" />
        <!-- tail (visible behind) -->
        <path d="M 118 100 C 138 92, 140 72, 124 64 C 116 72, 112 88, 118 100 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" stroke-linejoin="round" />
        <path d="M 124 64 Q 130 68, 128 74" fill="none" stroke="#fde2c4" stroke-width="3" stroke-linecap="round" />
        <!-- head -->
        <ellipse cx="80" cy="62" rx="30" ry="26" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" />
        <!-- ears -->
        <path d="M 54 46 L 58 28 L 70 42 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" stroke-linejoin="round" />
        <path d="M 106 46 L 102 28 L 90 42 Z" fill="#ec7c34" stroke="#a64a14" stroke-width="1.5" stroke-linejoin="round" />
        <path d="M 58 36 L 60 42 L 64 39 Z" fill="#7a3a14" />
        <path d="M 102 36 L 100 42 L 96 39 Z" fill="#7a3a14" />
        <!-- face mask -->
        <ellipse cx="80" cy="70" rx="18" ry="13" fill="#fde2c4" />
        <!-- eyes -->
        <ellipse cx="68" cy="60" rx="3.2" ry="3.8" fill="#2b2218" />
        <ellipse cx="92" cy="60" rx="3.2" ry="3.8" fill="#2b2218" />
        <circle cx="67" cy="59" r="1.1" fill="#ffffff" />
        <circle cx="91" cy="59" r="1.1" fill="#ffffff" />
        <!-- nose + mouth -->
        <ellipse cx="80" cy="71" rx="2.6" ry="2" fill="#2b2218" />
        <path d="M 80 73 Q 80 78, 76 78" fill="none" stroke="#2b2218" stroke-width="1.4" stroke-linecap="round" />
        <path d="M 80 73 Q 80 78, 84 78" fill="none" stroke="#2b2218" stroke-width="1.4" stroke-linecap="round" />
      </svg>
      <p class="empty-title">No transcripts yet</p>
      <p class="empty-body">Hold <kbd>F8</kbd> anywhere on your computer to dictate. Your recording will land here.</p>
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

  /* Filter pills — v0.4.0 design playbook. Individual rounded pills with
     a soft cream fill by default and a vibrant orange fill when active.
     No segmented-control container — each pill is its own button. */
  .filter-pills {
    display: flex;
    gap: 6px;
  }

  .pill {
    background: var(--bg-card);
    border: 1px solid var(--border);
    padding: 5px 14px;
    font-size: 12px;
    color: var(--text-secondary);
    border-radius: 999px;
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
    font-weight: 500;
    font-family: inherit;
  }

  .pill:hover {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }

  .pill.active {
    background: var(--accent);
    color: #ffffff;
    border-color: var(--accent);
    box-shadow: 0 1px 2px rgba(184, 84, 18, 0.25);
  }

  .pill.error-pill.active {
    background: var(--danger);
    border-color: var(--danger);
    color: #ffffff;
    box-shadow: 0 1px 2px rgba(168, 58, 42, 0.30);
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
    gap: 4px;
  }

  /* Placeholder fox SVGs — to be replaced by user-provided pastoral
     illustrations in v0.4.1+. The SVG is built inline so it inherits the
     theme palette and shows up even before the real asset lands. */
  .empty-fox {
    margin-bottom: 14px;
    filter: drop-shadow(0 4px 8px rgba(120, 80, 30, 0.18));
  }
  .empty-fox.sleeping {
    animation: fox-breathe 3.4s ease-in-out infinite;
  }
  @keyframes fox-breathe {
    0%, 100% { transform: translateY(0) scale(1); }
    50%      { transform: translateY(-2px) scale(1.01); }
  }

  .empty-title {
    font-size: 17px;
    color: var(--text-primary);
    margin: 0 0 6px;
    font-weight: 600;
  }

  .empty-body {
    font-size: 13px;
    margin: 0;
    max-width: 320px;
    line-height: 1.5;
  }

  .empty-body kbd {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 5px;
    padding: 1px 6px;
    font-family: ui-monospace, "SF Mono", Cascadia, Consolas, monospace;
    font-size: 11px;
    color: var(--text-primary);
    margin: 0 2px;
  }
</style>
