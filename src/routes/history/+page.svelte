<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { history } from "$lib/history-store.svelte";
  import { api, type Recording } from "$lib/api";
  import HistoryRow from "$lib/HistoryRow.svelte";
  import UploadDialog from "$lib/UploadDialog.svelte";
  import StatusStrip from "$lib/StatusStrip.svelte";
  import Button from "$lib/ui/Button.svelte";
  import Kbd from "$lib/ui/Kbd.svelte";
  import { settings } from "$lib/settings-store.svelte";
  import { account } from "$lib/account-store.svelte";
  import { fleet } from "$lib/fleet-store.svelte";
  import { prettyHotkey } from "$lib/hotkey-display";
  import { showMessage } from "$lib/dialogs";

  type FilterId = "all" | "light" | "drafting" | "meeting" | "upload" | "error";
  let filter = $state<FilterId>("all");
  // Filters name what the user thinks about, not the internal mode enum.
  const FILTERS: { id: FilterId; label: string }[] = [
    { id: "all", label: "All" },
    { id: "light", label: "Dictations" },
    { id: "drafting", label: "Drafts" },
    { id: "meeting", label: "Meetings" },
    { id: "upload", label: "Uploads" },
    { id: "error", label: "Failed" },
  ];
  let searchEl = $state<HTMLInputElement | null>(null);
  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform ?? "");
  function onGlobalKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      searchEl?.focus();
      searchEl?.select();
    }
  }
  let search = $state("");

  // Audio upload: staged file paths + modal state. `stagedPaths` is shared with
  // UploadDialog so a window-wide drag-drop drops files straight into it.
  const AUDIO_EXTS = ["wav", "mp3", "m4a", "aac", "ogg", "oga", "opus", "flac", "webm", "mp4"];
  let uploadOpen = $state(false);
  let stagedPaths = $state<string[]>([]);
  let dragActive = $state(false);

  function isAudioPath(p: string): boolean {
    return AUDIO_EXTS.includes((p.split(".").pop() || "").toLowerCase());
  }

  function openUpload() {
    stagedPaths = [];
    uploadOpen = true;
  }

  onMount(() => {
    history.refresh();
    history.subscribe();
    account.init();
    // Device identities for the per-card chips. Best-effort: signed out, this
    // resolves to an empty fleet and the chips simply never render.
    void fleet.subscribe();

    // Window-wide drag-and-drop. Tauri delivers real filesystem paths here
    // (HTML5 drag-drop in a webview can't see them), so this is the reliable
    // "drop an audio file onto the app" path. Dropping audio opens the upload
    // dialog pre-staged with the files.
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter") {
          // Only the "enter" (and "drop") payloads carry `paths`; "over" does
          // not. Reading it there is a type error, so gate on "enter" only —
          // dragActive stays sticky through the subsequent "over" events.
          dragActive = (p.paths ?? []).some(isAudioPath) || dragActive;
        } else if (p.type === "leave") {
          dragActive = false;
        } else if (p.type === "drop") {
          dragActive = false;
          const audio = (p.paths ?? []).filter(isAudioPath);
          if (audio.length > 0) {
            stagedPaths = audio;
            uploadOpen = true;
          }
        }
      })
      .then((u) => (unlisten = u))
      .catch(() => {});
    return () => unlisten?.();
  });

  let filtered = $derived.by(() => {
    let list = history.list;
    if (filter !== "all") {
      list = list.filter((r) => {
        switch (filter) {
          case "error": return r.status === "error";
          case "meeting": return r.is_meeting;
          case "upload": return r.source === "upload";
          case "light": return r.mode === "light" || r.mode === "advanced";
          default: return r.mode === filter;
        }
      });
    }
    const q = search.trim().toLowerCase();
    if (q) {
      list = list.filter((r) => {
        const t = [r.title, r.transcript, r.cleaned_text, r.drafted_text]
          .filter(Boolean)
          .join(" ")
          .toLowerCase();
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

  async function openRecordingsFolder() {
    try {
      await api.revealFolder("audio");
    } catch (e) {
      await showMessage(`Could not open folder: ${e}`);
    }
  }
</script>

<svelte:window onkeydown={onGlobalKey} />

<section class="history">
  <header class="history-head">
    <div class="head-row">
      <h1 class="page-title">Home</h1>

      <div class="search-row">
        <svg class="search-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.8" />
          <path d="M 10.5 10.5 L 14 14" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
        </svg>
        <input
          type="search"
          class="search"
          placeholder="Search your notes…"
          bind:value={search}
          bind:this={searchEl}
        />
        {#if search}
          <button class="search-clear" onclick={() => (search = "")} aria-label="Clear search">×</button>
        {:else}
          <span class="search-kbd"><Kbd combo={isMac ? "Cmd+K" : "Ctrl+K"} /></span>
        {/if}
      </div>

      <div class="head-actions">
        <Button tone="secondary" onclick={openUpload} title="Transcribe an audio file from your computer or phone">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path d="M8 10V2M5 5l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M3 10v2.5a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          Upload
        </Button>
        <Button tone="ghost" iconOnly aria-label="Open recordings folder" title="Open recordings folder in file manager" onclick={openRecordingsFolder}>
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
            <path d="M 2 4 L 2 12 A 1 1 0 0 0 3 13 L 13 13 A 1 1 0 0 0 14 12 L 14 5 A 1 1 0 0 0 13 4 L 8 4 L 6.5 2.5 A 1 1 0 0 0 5.8 2.2 L 3 2.2 A 1 1 0 0 0 2 3.2 Z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
          </svg>
        </Button>
        <Button tone="ghost" iconOnly aria-label="Reload list" title="Reload the list from disk — does not re-transcribe anything" onclick={() => history.refresh()}>
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
            <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
          </svg>
        </Button>
      </div>
    </div>

    <StatusStrip />

    <div class="filter-pills" role="tablist" aria-label="Filter notes">
      {#each FILTERS as f (f.id)}
        <button class="pill" class:active={filter === f.id} class:error-pill={f.id === "error"} role="tab" aria-selected={filter === f.id} onclick={() => (filter = f.id)}>
          {f.label}{#if filter === f.id && search}<span class="pill-count">{filtered.length}</span>{/if}
        </button>
      {/each}
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
        </div>
        {#each group.items as rec (rec.id)}
          <HistoryRow {rec} />
        {/each}
      {/each}
    </div>
  {/if}

  {#if dragActive}
    <!-- Full-pane hint while an audio file is dragged over the window. -->
    <div class="drag-overlay" aria-hidden="true">
      <div class="drag-card">
        <svg viewBox="0 0 24 24" width="40" height="40" aria-hidden="true">
          <path d="M12 16V4M8 8l4-4 4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M4 16v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <p>Drop to transcribe</p>
      </div>
    </div>
  {/if}

  <UploadDialog bind:open={uploadOpen} bind:paths={stagedPaths} />


  <!-- Ambient pastoral meadow pinned to the bottom of the pane (design
       playbook: "the bottom sticky wave"). Sits BEHIND the row cards
       (z-index 0 vs the content's 1) so the last cards float over the
       hills as you reach the end of the list. pointer-events: none —
       pure decoration, never intercepts clicks or scroll. -->
  <div class="history-meadow" aria-hidden="true">
    <img src="/fox/meadow-strip.svg" alt="" />
  </div>
</section>

<style>
  .history {
    padding: 0;
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
    /* Anchor for the bottom meadow strip. */
    position: relative;
  }

  /* Header sits directly on the cream surface (design playbook) — no
     elevated white block, no hard border. The page reads as one warm
     canvas with cards floating on it. */
  .history-head {
    padding: 22px 28px 10px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex-shrink: 0;
    position: relative;
    z-index: 1;
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
    border-radius: var(--radius-md);
    padding: 10px 30px 10px 34px;
    font-size: 13px;
    background: color-mix(in srgb, var(--bg-card) 92%, transparent);
    box-shadow: var(--shadow-xs);
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


  /* Drag-over hint — appears while an audio file is dragged onto the window. */
  .drag-overlay {
    position: absolute;
    inset: 0;
    z-index: 150;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(250, 246, 236, 0.72);
    backdrop-filter: blur(2px);
    pointer-events: none;
  }
  :global(body[data-theme="dark"]) .drag-overlay {
    background: rgba(30, 24, 16, 0.72);
  }
  .drag-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 32px 44px;
    border: 2px dashed var(--accent);
    border-radius: 18px;
    background: var(--bg-card);
    color: var(--accent);
    box-shadow: 0 16px 50px rgba(60, 40, 15, 0.25);
  }
  .drag-card p {
    margin: 0;
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
  }

  /* Press-and-hold-to-clear button. Quiet ghost at rest (neutral secondary
     colour, transparent), turning danger-red only on hover / while holding.
     A danger fill sweeps left→right over 3s while held; releasing cancels. */

  /* Date group headers — label + count chip, no rule line (the card gaps
     already separate groups visually, per the playbook mock). */
  .date-divider {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 4px 2px;
    background: var(--bg-surface);
    position: sticky;
    top: 0;
    z-index: 2;
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

  /* Card stack. Generous bottom padding so the last card can scroll clear
     of the meadow strip pinned underneath. */
  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 2px 24px 130px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    position: relative;
    z-index: 1;
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
    position: relative;
    z-index: 1;
  }

  /* Pastoral meadow pinned to the bottom of the pane — the playbook's
     ambient "wave" footer. Low opacity + soft top fade so it reads as
     atmosphere; cards scroll over it (it's z-index 0 under the content). */
  .history-meadow {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 130px;
    overflow: hidden;
    pointer-events: none;
    opacity: 0.8;
    z-index: 0;
    mask-image: linear-gradient(to bottom, transparent 0%, #000 45%, #000 100%);
    -webkit-mask-image: linear-gradient(to bottom, transparent 0%, #000 45%, #000 100%);
  }
  .history-meadow img {
    width: 100%;
    height: 100%;
    display: block;
    object-fit: cover;
    object-position: center bottom;
  }

  /* The meadow is painted for the light cream theme; in the dark themes a
     pastel-green strip would glow, so dim it to a faint silhouette. Covers
     explicit dark AND auto-following-system-dark. */
  :global(body[data-theme="dark"]) .history-meadow {
    opacity: 0.22;
  }
  @media (prefers-color-scheme: dark) {

    :global(body[data-theme="auto"]) .history-meadow {
      opacity: 0.22;
    }
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

  @media (max-width: 780px) {

    .history-head {
      padding: 18px 18px 10px;
    }

    .rows {
      padding-inline: 18px;
    }
  }

  .head-row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .page-title {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: var(--lh-tight);
    color: var(--text-primary);
  }
  .head-row .search-row {
    flex: 1;
    max-width: 520px;
    margin: 0 auto;
  }
  .search-kbd {
    position: absolute;
    right: 10px;
    top: 50%;
    transform: translateY(-50%);
    opacity: 0.7;
    pointer-events: none;
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    margin-left: auto;
  }
  .pill-count {
    margin-left: 6px;
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--bg-card);
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }
</style>
