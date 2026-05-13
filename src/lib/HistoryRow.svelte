<script lang="ts">
  import { tick } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { history } from "./history-store.svelte";
  import { api, type Recording } from "./api";

  let { rec } = $props<{ rec: Recording }>();

  type Tab = "raw" | "cleaned" | "drafted";

  // Active tab. Defaults to the "most refined" version that exists:
  // drafted > cleaned > raw. User can switch by clicking the tabs.
  function defaultTab(r: Recording): Tab {
    if (r.drafted_text) return "drafted";
    if (r.cleaned_text) return "cleaned";
    return "raw";
  }
  let activeTab = $state<Tab>(defaultTab(rec));

  let expanded = $state(false);
  let audioUrl = $state<string | null>(null);
  let audioEl = $state<HTMLAudioElement | null>(null);
  let playing = $state(false);
  let busy = $state(false);

  // When the generate-on-demand command for THIS row is in flight, track
  // which kind so the corresponding tab can show a spinner instead of
  // letting the user fire a second request.
  let generating = $state<null | "cleaned" | "drafted">(null);

  let displayedText = $derived.by(() => {
    if (activeTab === "drafted") return rec.drafted_text || "(no draft yet)";
    if (activeTab === "cleaned") return rec.cleaned_text || "(no cleaned version yet)";
    return rec.transcript || "(no transcript)";
  });
  let isError = $derived(rec.status === "error");
  // Retry must be available at ALL statuses — including 'transcribing'
  // and 'cleaning'. The whole reason this exists is to recover from
  // recordings that got stranded mid-flow (the flow crashed before
  // status flipped to 'error' or 'done'). Disabling on status would
  // lock the user out of the exact case they need to fix.
  //
  // Rust retry_recording is idempotent: bumps retry_count, resets
  // status, re-runs. If a real job somehow IS still running, the
  // second pass wins on UPSERT — annoying but not catastrophic.
  // Only block while THIS row's button has a request in flight.
  let retryDisabled = $derived(busy);

  // Inspector — (i) button. Shows: full error, retry count, providers
  // used, Clippy note, audio path. Surfaced for every recording so the
  // user can see what happened on success too. Red dot when there's an
  // error worth noticing.
  let showInspector = $state(false);
  let inspectorHasNews = $derived(isError || !!rec.error);

  // Kebab (3-dot) menu — holds Retry + Delete per the v0.4.0 design
  // playbook. Closes on outside click or Escape.
  let kebabOpen = $state(false);
  $effect(() => {
    if (!kebabOpen) return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && target.closest(".kebab-wrap")) return;
      kebabOpen = false;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") kebabOpen = false;
    };
    window.addEventListener("click", onDoc);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", onDoc);
      window.removeEventListener("keydown", onKey);
    };
  });

  function timeShort(iso: string): string {
    try {
      const d = new Date(iso);
      const now = new Date();
      const diff = now.getTime() - d.getTime();
      const sameDay = d.toDateString() === now.toDateString();
      if (diff < 60_000) return "just now";
      if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
      if (sameDay) return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      const yesterday = new Date(now);
      yesterday.setDate(yesterday.getDate() - 1);
      const isYesterday = d.toDateString() === yesterday.toDateString();
      if (isYesterday)
        return `Yesterday ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
      return d.toLocaleString([], {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return iso;
    }
  }

  function durationShort(ms: number): string {
    if (!ms) return "—";
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    return `${m}m ${s % 60}s`;
  }

  async function ensureAudioUrl() {
    if (audioUrl) return;
    try {
      // Use the data: URL command — bypasses Tauri's asset protocol entirely.
      // Backend reads the WAV file and returns base64. Heavier on memory than
      // streaming but bulletproof on Windows where the asset protocol scope
      // doesn't play nice with AppData paths.
      audioUrl = await api.audioDataUrlFor(rec.id);
      console.log("[audio] data URL loaded for", rec.id, "size=", audioUrl.length);
    } catch (e) {
      console.error("[audio] audioDataUrlFor failed", e);
    }
  }

  async function togglePlay() {
    await ensureAudioUrl();
    // Wait for Svelte to render the <audio> element (it's gated by {#if audioUrl}).
    await tick();
    if (!audioEl) {
      console.warn("[audio] audio element not yet bound after tick — retrying once");
      await tick();
    }
    if (!audioEl) {
      console.error("[audio] audio element still null, aborting");
      return;
    }
    if (audioEl.paused) {
      try {
        await audioEl.play();
        playing = true;
      } catch (e) {
        console.error("[audio] play() failed", e);
      }
    } else {
      audioEl.pause();
      playing = false;
    }
  }

  async function copyText() {
    await writeText(displayedText);
  }

  /// Click handler for the Cleaned / Drafted tabs. If the version exists,
  /// just switch the active tab. If it doesn't, fire the on-demand
  /// generate command, wait for the result, then switch to it. Errors
  /// surface as a simple alert — the row already shows status info via
  /// the (i) inspector, no need to over-engineer.
  async function onTabClick(kind: "cleaned" | "drafted") {
    if (kind === "cleaned" && rec.cleaned_text) { activeTab = "cleaned"; return; }
    if (kind === "drafted" && rec.drafted_text) { activeTab = "drafted"; return; }
    if (!rec.transcript) {
      alert("No raw transcript yet — retry the recording first.");
      return;
    }
    generating = kind;
    try {
      await api.generateAltVersion(rec.id, kind);
      // Pull the freshly-generated text into our local rec via a refresh.
      // The history-store refresh re-fetches and re-renders the list.
      await history.refresh();
      activeTab = kind;
    } catch (e) {
      alert(`Could not generate ${kind} version: ${e}`);
    } finally {
      generating = null;
    }
  }

  async function remove() {
    if (!confirm("Delete this recording (text + audio)?")) return;
    busy = true;
    try {
      await history.remove(rec.id);
    } finally {
      busy = false;
    }
  }

  async function retry() {
    // On non-failed rows, confirm before nuking the existing transcript.
    // Done rows are the easy mis-click target — "I'll just check this
    // recording" → accidentally re-burn an STT call.
    if (!isError) {
      const ok = confirm(
        "Re-run transcription on this recording? The current transcript and cleaned text will be replaced.",
      );
      if (!ok) return;
    }
    busy = true;
    try {
      await api.retryRecording(rec.id);
      await history.refresh();
    } catch (e) {
      alert(`Retry failed: ${e}`);
    } finally {
      busy = false;
    }
  }

  function fmtFullTime(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleString([], {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    } catch {
      return iso;
    }
  }
</script>

<!-- Whole row is click-to-expand. Action buttons (play/copy/retry/info)
     and the body's own action area stopPropagation so they don't toggle
     expansion when clicked. The caret stays as a visual affordance but is
     not the only thing that toggles — clicking anywhere on the body or the
     meta strip also works (matches the user's mental model: "this row is a
     thing I tap to read"). -->
<article
  class="row"
  class:expanded
  class:error-row={isError}
  role="button"
  tabindex="0"
  aria-expanded={expanded}
  onclick={() => (expanded = !expanded)}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      expanded = !expanded;
    }
  }}
>
  <header class="row-head">
    <button class="row-toggle" onclick={(e) => { e.stopPropagation(); expanded = !expanded; }} aria-label="Toggle expand">
      <svg class="caret" class:open={expanded} viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
        <path d="M 5 3 L 11 8 L 5 13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>
    <div class="meta">
      <span class="when">{timeShort(rec.created_at)}</span>
      <span class="dot">·</span>
      <span class="dur">{durationShort(rec.duration_ms)}</span>

      <!-- Version tabs inline with the metadata row. Compact, iOS-style
           segmented control. The original mode badge ("light"/"drafting")
           is gone — the active tab tells you the same thing without
           wasting vertical space below the body. Click stops propagation
           so a tab click doesn't also toggle row expansion. -->
      {#if rec.transcript || rec.cleaned_text || rec.drafted_text}
        <span class="tabs-inline" role="presentation" onclick={(e) => e.stopPropagation()}>
          <button
            class="tab"
            class:active={activeTab === "raw"}
            class:dim={!rec.transcript}
            disabled={!rec.transcript}
            onclick={() => (activeTab = "raw")}
            title="Raw transcript"
          >Raw</button>
          <button
            class="tab"
            class:active={activeTab === "cleaned"}
            class:dim={!rec.cleaned_text}
            class:loading={generating === "cleaned"}
            disabled={generating !== null}
            onclick={() => onTabClick("cleaned")}
            title={rec.cleaned_text ? "Cleaned" : "Generate a cleaned version"}
          >{generating === "cleaned" ? "…" : "Cleaned"}</button>
          <button
            class="tab"
            class:active={activeTab === "drafted"}
            class:dim={!rec.drafted_text}
            class:loading={generating === "drafted"}
            disabled={generating !== null}
            onclick={() => onTabClick("drafted")}
            title={rec.drafted_text ? "Drafted" : "Generate a drafted version"}
          >{generating === "drafted" ? "…" : "Drafted"}</button>
        </span>
      {/if}

      {#if rec.retry_count > 0}
        <span class="retry-count" title="Number of retry attempts">↻ {rec.retry_count}</span>
      {/if}
      {#if isError}
        <span class="err-pill">Failed — see details</span>
      {/if}

      <!-- (i) details button. Always present; pulses a red dot when
           there's an error to surface so the user notices without
           clicking. Clicking expands an inline details panel below
           the body. -->
      <button
        class="info-btn"
        class:has-news={inspectorHasNews}
        onclick={(e) => { e.stopPropagation(); showInspector = !showInspector; }}
        aria-label="Show recording details and event log"
        aria-expanded={showInspector}
        title={inspectorHasNews ? "Details (error logged)" : "Details"}
      >
        i
      </button>
    </div>

    <div class="actions" onclick={(e) => e.stopPropagation()} role="presentation">
      <button class="action-btn play" onclick={togglePlay} disabled={busy} title="Play / pause audio">
        {#if playing}
          <svg viewBox="0 0 16 16" width="14" height="14"><rect x="4" y="3" width="3" height="10" fill="currentColor"/><rect x="9" y="3" width="3" height="10" fill="currentColor"/></svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14"><path d="M 5 3 L 13 8 L 5 13 Z" fill="currentColor"/></svg>
        {/if}
      </button>

      <!-- Copy: only meaningful when there's actual text. Hidden on
           rows that errored before producing a transcript. -->
      {#if !isError || rec.transcript || rec.cleaned_text}
        <button class="action-btn" onclick={copyText} disabled={busy} title="Copy text">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <rect x="4" y="3" width="8" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.6"/>
            <rect x="2.5" y="1.5" width="8" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.4" opacity="0.55"/>
          </svg>
        </button>
      {/if}

      <!-- 3-dot kebab menu — destination for less-frequently-used row
           actions (Retry, Delete) per the v0.4.0 design playbook. Retry
           used to be a top-level button; on the new layout we keep Play
           + Copy prominent and tuck the rest behind this menu. -->
      <div class="kebab-wrap">
        <button
          class="action-btn kebab"
          class:emphasized={isError}
          onclick={(e) => { e.stopPropagation(); kebabOpen = !kebabOpen; }}
          aria-haspopup="menu"
          aria-expanded={kebabOpen}
          title="More"
        >
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <circle cx="8" cy="3" r="1.6" fill="currentColor" />
            <circle cx="8" cy="8" r="1.6" fill="currentColor" />
            <circle cx="8" cy="13" r="1.6" fill="currentColor" />
          </svg>
        </button>
        {#if kebabOpen}
          <div class="kebab-menu" role="menu" onclick={(e) => e.stopPropagation()}>
            <button
              class="kebab-item"
              class:emphasized={isError}
              role="menuitem"
              disabled={retryDisabled}
              onclick={() => { kebabOpen = false; retry(); }}
            >
              <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
              </svg>
              <span>{isError ? "Retry" : "Re-run transcription"}</span>
            </button>
            <button
              class="kebab-item danger"
              role="menuitem"
              disabled={busy}
              onclick={() => { kebabOpen = false; remove(); }}
            >
              <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                <path d="M 3 4 L 13 4 M 5 4 V 13 A 1 1 0 0 0 6 14 H 10 A 1 1 0 0 0 11 13 V 4 M 7 7 V 11 M 9 7 V 11 M 6 4 V 3 A 1 1 0 0 1 7 2 H 9 A 1 1 0 0 1 10 3 V 4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
              </svg>
              <span>Delete recording</span>
            </button>
          </div>
        {/if}
      </div>
    </div>
  </header>

  <div class="body" class:clamped={!expanded}>
    {#if isError}
      <div class="err">
        <strong>Failed:</strong> {rec.error || "unknown error"}
      </div>
    {/if}
    <p class="text">{displayedText}</p>
    {#if expanded && rec.clippy_note}
      <p class="note">Clippy note: {rec.clippy_note}</p>
    {/if}
    {#if expanded}
      <!-- Expanded action area stops click-bubbling so its buttons don't
           re-toggle the row when clicked. -->
      <div role="presentation" onclick={(e) => e.stopPropagation()}>
      <!-- Delete used to live in the action button row. Moved here so it
           takes a deliberate click instead of being a thumb-reachable
           danger button alongside Play/Copy/Retry. Still confirms. -->
      <div class="expanded-actions">
        <button class="delete-link" onclick={remove} disabled={busy}>Delete recording</button>
      </div>
      </div>
    {/if}
  </div>

  {#if showInspector}
    <!-- Inline details panel. Sits below the body so it doesn't cover
         anything; collapses cleanly without layout shift elsewhere. -->
    <div class="inspector" role="region" aria-label="Recording details">
      <div class="insp-grid">
        <div class="insp-k">Status</div>
        <div class="insp-v">
          <span class="insp-badge insp-badge-{rec.status}">{rec.status}</span>
        </div>

        {#if rec.error}
          <div class="insp-k">Last error</div>
          <div class="insp-v">
            <pre class="insp-err">{rec.error}</pre>
          </div>
        {/if}

        <div class="insp-k">Retries</div>
        <div class="insp-v">{rec.retry_count}</div>

        <div class="insp-k">Mode</div>
        <div class="insp-v">{rec.mode}</div>

        <div class="insp-k">Duration</div>
        <div class="insp-v">{durationShort(rec.duration_ms)}</div>

        <div class="insp-k">STT provider</div>
        <div class="insp-v insp-mono">{rec.stt_provider ?? "—"}</div>

        <div class="insp-k">LLM provider</div>
        <div class="insp-v insp-mono">{rec.llm_provider ?? "—"}</div>

        {#if rec.clippy_note}
          <div class="insp-k">Clippy note</div>
          <div class="insp-v">{rec.clippy_note}</div>
        {/if}

        <div class="insp-k">Created</div>
        <div class="insp-v">{fmtFullTime(rec.created_at)}</div>

        <div class="insp-k">Audio</div>
        <div class="insp-v insp-mono insp-small">{rec.audio_path}</div>

        <div class="insp-k">ID</div>
        <div class="insp-v insp-mono insp-small">{rec.id}</div>
      </div>
    </div>
  {/if}

  <!-- (Old "Polished / Base" toggle removed — replaced by the 3-tab bar
       at the top of the body.) -->

  {#if audioUrl}
    <audio
      bind:this={audioEl}
      src={audioUrl}
      onended={() => (playing = false)}
      onpause={() => (playing = false)}
      onplay={() => (playing = true)}
      class="hidden-audio"
    ></audio>
  {/if}
</article>

<style>
  .row {
    display: flex;
    flex-direction: column;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border-subtle);
    transition: background 120ms ease;
    position: relative;
    cursor: pointer;
  }

  .row:hover {
    background: var(--bg-subtle);
  }

  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .row.error-row {
    background: var(--danger-fade);
  }

  .row-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .row-toggle {
    background: transparent;
    border: 1px solid transparent;
    cursor: pointer;
    color: var(--text-secondary);
    padding: 6px;
    border-radius: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background 100ms ease, border-color 100ms ease;
  }

  .row-toggle:hover {
    background: var(--bg-subtle);
    border-color: var(--border);
    color: var(--text-primary);
  }

  .caret {
    transition: transform 180ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  .caret.open {
    transform: rotate(90deg);
  }

  .meta {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
    min-width: 0;
    flex-wrap: wrap;
  }

  .when {
    font-weight: 500;
    color: var(--text-primary);
  }

  .dur, .retry-count {
    color: var(--text-secondary);
  }

  .dot {
    color: var(--border);
    flex-shrink: 0;
  }

  .mode {
    padding: 2px 9px;
    border-radius: 9999px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .mode.light {
    background: var(--accent-fade);
    color: var(--accent);
  }

  .mode.advanced {
    background: rgba(175, 82, 222, 0.18);
    color: #af52de;
  }

  .mode.drafting {
    background: rgba(255, 159, 10, 0.20);
    color: #c47a30;
  }

  .err-pill {
    background: var(--danger-fade);
    color: var(--danger);
    padding: 2px 8px;
    border-radius: 9999px;
    font-size: 10px;
    font-weight: 500;
  }

  .retry-count {
    background: var(--bg-subtle);
    padding: 1px 7px;
    border-radius: 9999px;
    font-size: 10px;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .action-btn {
    background: var(--bg-card);
    border: 1px solid var(--border);
    cursor: pointer;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    color: var(--text-primary);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: all 120ms ease;
  }

  .action-btn:hover:not(:disabled) {
    background: var(--bg-subtle);
    border-color: var(--text-secondary);
    transform: translateY(-1px);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .action-btn.play:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }

  .action-btn.delete:hover:not(:disabled) {
    background: var(--danger-fade);
    border-color: var(--danger);
    color: var(--danger);
  }

  /* Retry is now always-visible. Default look matches the other action
     buttons (subtle, monochrome). `emphasized` is added when the row
     errored, making Retry the obvious recovery action. */
  .action-btn.retry:hover:not(:disabled) {
    color: var(--warning, var(--accent));
    border-color: var(--warning, var(--accent));
  }

  /* Kebab (3-dot menu) button + popover. Sits at the right end of the
     row's action group; opens a small popover with Retry + Delete. */
  .kebab-wrap {
    position: relative;
  }
  .action-btn.kebab:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }
  .action-btn.kebab.emphasized {
    color: var(--warning);
    border-color: var(--warning);
    background: var(--warning-fade);
  }
  .kebab-menu {
    position: absolute;
    right: 0;
    top: 100%;
    margin-top: 6px;
    min-width: 180px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgba(120, 80, 30, 0.18);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    z-index: 50;
  }
  .kebab-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    background: transparent;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-primary);
    cursor: pointer;
    text-align: left;
    font-family: inherit;
  }
  .kebab-item:hover:not(:disabled) {
    background: var(--bg-subtle);
  }
  .kebab-item:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .kebab-item.danger {
    color: var(--danger);
  }
  .kebab-item.danger:hover:not(:disabled) {
    background: var(--danger-fade);
  }
  .kebab-item.emphasized {
    color: var(--warning);
    font-weight: 500;
  }

  .action-btn.retry.emphasized {
    background: var(--warning-fade);
    border-color: var(--warning);
    color: var(--warning);
  }
  .action-btn.retry.emphasized:hover:not(:disabled) {
    background: var(--warning);
    color: #fff;
  }

  /* Round (i) details button. Italic serif "i" — the classic affordance.
     Pulses a red dot in the top-right when there's an error to surface. */
  .info-btn {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-secondary);
    font-family: Georgia, "Times New Roman", serif;
    font-style: italic;
    font-size: 12px;
    line-height: 16px;
    padding: 0;
    cursor: pointer;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .info-btn:hover {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }
  .info-btn.has-news::after {
    content: "";
    position: absolute;
    top: -2px;
    right: -2px;
    width: 6px;
    height: 6px;
    background: var(--danger);
    border-radius: 50%;
    border: 1px solid var(--bg-elev);
  }

  /* Delete moved out of the icon button row into a subtle text link
     that only appears when the row is expanded. Less mis-click surface. */
  .expanded-actions {
    margin-top: 10px;
    display: flex;
    justify-content: flex-end;
  }
  .delete-link {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
  }
  .delete-link:hover:not(:disabled) {
    color: var(--danger);
    text-decoration-color: var(--danger);
  }
  .delete-link:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* Inspector panel — inline expansion below the row body. Two-column
     key/value grid; the error text gets its own monospace block. */
  .inspector {
    margin: 10px 0 0 36px;
    padding: 12px 14px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
  }
  .insp-grid {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 6px 12px;
    font-size: 12px;
    align-items: baseline;
  }
  .insp-k {
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 10px;
    font-weight: 600;
  }
  .insp-v {
    color: var(--text-primary);
    overflow-wrap: anywhere;
  }
  .insp-v.insp-mono {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
  .insp-v.insp-small {
    font-size: 11px;
  }
  .insp-badge {
    display: inline-block;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 1px 7px;
    border-radius: 9999px;
    border: 1px solid var(--border);
  }
  .insp-badge-done {
    background: var(--accent-fade);
    color: var(--accent);
    border-color: var(--accent);
  }
  .insp-badge-error {
    background: var(--danger-fade);
    color: var(--danger);
    border-color: var(--danger);
  }
  .insp-badge-recording,
  .insp-badge-transcribing,
  .insp-badge-cleaning,
  .insp-badge-injecting {
    background: var(--bg-card);
    color: var(--text-secondary);
  }
  .insp-err {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px;
    color: var(--danger);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 200px;
    overflow-y: auto;
  }

  .body {
    margin-top: 10px;
    padding-left: 36px;
  }

  .body.clamped .text {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* Inline version tabs — Raw / Cleaned / Drafted. v0.4.0 design playbook
     gives them their own visual identity: the active tab is filled with
     the accent orange + white text (the playbook's "Cleaned" pill), other
     tabs are subtle cream-fill outlines. Dim (= not yet generated) tabs
     are translucent and italic to read as a CTA, not data. */
  .tabs-inline {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: 8px;
  }

  .tab {
    background: var(--bg-card);
    border: 1px solid var(--border);
    padding: 2px 10px;
    font-size: 10px;
    line-height: 1.5;
    font-weight: 500;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: 999px;
    transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
    font-family: inherit;
    letter-spacing: 0.01em;
  }

  .tab:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }

  .tab.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #ffffff;
    box-shadow: 0 1px 2px rgba(184, 84, 18, 0.25);
  }
  .tab.active:hover {
    background: var(--accent-hover, var(--accent));
    border-color: var(--accent-hover, var(--accent));
    color: #ffffff;
  }

  /* Dim = version doesn't exist yet; still a click target. */
  .tab.dim {
    background: transparent;
    color: var(--text-secondary);
    opacity: 0.6;
    font-style: italic;
    border-style: dashed;
  }
  .tab.dim:hover:not(:disabled) {
    opacity: 1;
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-fade);
  }

  .tab.loading {
    color: var(--accent);
    opacity: 1;
    font-style: italic;
    border-color: var(--accent);
    background: var(--accent-fade);
  }

  .tab:disabled {
    cursor: not-allowed;
  }

  .text {
    font-size: 14px;
    line-height: 1.55;
    color: var(--text-primary);
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* Compact variant toggle — inline pill, tiny chevrons. */
  .variant-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 6px 0 0 36px;
    padding: 3px 8px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: 9999px;
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 11px;
    transition: all 120ms ease;
  }
  .variant-toggle:hover {
    background: var(--accent-fade);
    color: var(--accent);
    border-color: var(--accent);
  }
  .variant-name-compact {
    font-weight: 600;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .err {
    color: #b3261e;
    font-size: 12px;
    margin-bottom: 4px;
  }

  .note {
    color: #ff9f0a;
    font-size: 11px;
    margin: 6px 0 0;
  }

  .hidden-audio {
    display: none;
  }
</style>
