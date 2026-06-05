<script lang="ts">
  import { onMount } from "svelte";
  import { history } from "$lib/history-store.svelte";
  import { api, type Recording } from "$lib/api";
  import HistoryRow from "$lib/HistoryRow.svelte";
  import StatsWidget from "$lib/StatsWidget.svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { settings } from "$lib/settings-store.svelte";
  import { prettyHotkey } from "$lib/hotkey-display";

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

  // Press-and-hold-to-confirm clear. Holding the button for HOLD_MS deletes
  // everything (DB rows + transcripts + the .wav files on disk). Releasing
  // early cancels — no accidental nukes, no modal dialog.
  const HOLD_MS = 3000;
  let holdActive = $state(false);
  let holdProgress = $state(0); // 0..1
  let clearing = $state(false);
  let clearedMsg = $state("");
  let _holdStart = 0;
  let _holdRAF: number | null = null;

  function holdTick() {
    const t = Math.min(1, (Date.now() - _holdStart) / HOLD_MS);
    holdProgress = t;
    if (t >= 1) {
      _holdRAF = null;
      void doClear();
      return;
    }
    _holdRAF = requestAnimationFrame(holdTick);
  }
  function startHold() {
    if (clearing) return;
    holdActive = true;
    clearedMsg = "";
    _holdStart = Date.now();
    _holdRAF = requestAnimationFrame(holdTick);
  }
  function cancelHold() {
    if (_holdRAF !== null) {
      cancelAnimationFrame(_holdRAF);
      _holdRAF = null;
    }
    holdActive = false;
    holdProgress = 0;
  }
  async function doClear() {
    cancelHold();
    clearing = true;
    try {
      const removed = await api.clearAllHistory();
      await history.refresh();
      clearedMsg = `Deleted ${removed} recording${removed === 1 ? "" : "s"} + audio files.`;
    } catch (e) {
      clearedMsg = `Clear failed: ${e}`;
    } finally {
      clearing = false;
      setTimeout(() => (clearedMsg = ""), 4000);
    }
  }

  async function openRecordingsFolder() {
    try {
      await api.revealFolder("audio");
    } catch (e) {
      alert(`Could not open folder: ${e}`);
    }
  }
</script>

<section class="history">
  <!-- Lifetime analytics at-a-glance — time saved, words, sessions, streak +
       a 14-day sparkline. Click through to the full /stats dashboard. Renders
       only once there's at least one recording. -->
  <StatsWidget />

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
        <button
          class="hold-clear"
          class:armed={holdActive}
          disabled={clearing}
          style="--hold:{holdProgress}"
          onmousedown={startHold}
          onmouseup={cancelHold}
          onmouseleave={cancelHold}
          title="Press and hold 3 seconds to delete all recordings and audio files"
        >
          <span class="hold-fill"></span>
          <span class="hold-text">
            {#if clearing}Clearing…{:else if holdActive}Keep holding to delete…{:else}Hold to clear all{/if}
          </span>
        </button>
        {#if clearedMsg}<span class="cleared-msg">{clearedMsg}</span>{/if}
      </div>
    </div>
  </header>

  {#if history.loading && history.list.length === 0}
    <!-- Loading state: watercolor walking fox + caption. The breathe
         animation is replaced with a subtle bob so the fox feels alive
         while transcripts load. -->
    <div class="empty">
      <img class="empty-fox loading" src="/fox/fox-loading.png" alt="" />
      <p class="empty-title">Loading transcripts…</p>
    </div>
  {:else if history.list.length === 0}
    <!-- First-run empty state: watercolor sitting fox in plants from the
         design playbook. -->
    <div class="empty">
      <img class="empty-fox" src="/fox/fox-empty-state.png" alt="" />
      <p class="empty-title">No transcripts yet</p>
      <p class="empty-body">Hold <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd> anywhere on your computer to dictate. Your recording will land here.</p>
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
      <!-- Pastoral horizon at the very end of the list — soft visual
           full-stop that reads as "you've reached the end" without being
           a hard divider. Watercolor autumn hills from the design
           playbook; faded so it doesn't compete with the row text. -->
      <div class="history-horizon" aria-hidden="true">
        <img src="/fox/landscape-combined.png" alt="" />
      </div>
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

  /* Press-and-hold-to-clear button. A danger-tinted fill sweeps left→right
     over 3s while held; releasing early cancels. */
  .hold-clear {
    position: relative;
    overflow: hidden;
    isolation: isolate;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 7px 14px;
    cursor: pointer;
    font-size: 12px;
    color: var(--danger);
    user-select: none;
    transition: border-color 120ms ease, background 120ms ease;
  }
  .hold-clear:hover {
    border-color: var(--danger);
    background: var(--danger-fade);
  }
  .hold-clear:disabled {
    opacity: 0.7;
    cursor: default;
  }
  .hold-clear.armed {
    border-color: var(--danger);
    color: #fff;
  }
  .hold-fill {
    position: absolute;
    inset: 0;
    z-index: -1;
    transform-origin: left center;
    transform: scaleX(var(--hold, 0));
    background: var(--danger);
    transition: transform 60ms linear;
  }
  .hold-text {
    position: relative;
    white-space: nowrap;
  }
  .cleared-msg {
    font-size: 11px;
    color: var(--text-secondary);
    margin-left: 8px;
    align-self: center;
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

  /* Pastoral landscape banner at the bottom of the list. Subtle —
     low opacity, soft top fade, clamped height — so it reads as ambient
     decoration, not an actual UI element.
     The aspect-ratio + max-height combo prevents the first-paint layout
     shift that used to make the banner "overflow" momentarily on app
     launch (image loaded after the first frame, suddenly took its
     natural height, shoved everything around). */
  .history-horizon {
    margin-top: 40px;
    width: 100%;
    max-height: 220px;
    overflow: hidden;
    pointer-events: none;
    opacity: 0.7;
    aspect-ratio: 4 / 1;
    mask-image: linear-gradient(to bottom, transparent 0%, #000 30%, #000 100%);
    -webkit-mask-image: linear-gradient(to bottom, transparent 0%, #000 30%, #000 100%);
  }
  .history-horizon img {
    width: 100%;
    height: 100%;
    display: block;
    object-fit: cover;
    object-position: center bottom;
  }

  /* Watercolor fox illustrations from the design playbook (PNGs in
     /static/fox/). Sized large enough to feel like real characters,
     not tiny icons. */
  .empty-fox {
    width: 180px;
    height: 180px;
    object-fit: contain;
    margin-bottom: 14px;
    filter: drop-shadow(0 6px 14px rgba(120, 80, 30, 0.15));
  }
  .empty-fox.loading {
    animation: fox-bob 2.4s ease-in-out infinite;
  }
  @keyframes fox-bob {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50%      { transform: translateY(-6px) rotate(-1.5deg); }
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
