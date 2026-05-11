<script lang="ts">
  import { tick } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { history } from "./history-store.svelte";
  import { api, type Recording } from "./api";

  let { rec } = $props<{ rec: Recording }>();

  // Variant: 0 = polished (cleaned_text), 1 = base (raw transcript).
  // Default to polished if available; otherwise fall back to raw.
  let variant = $state<0 | 1>(rec.cleaned_text ? 0 : 1);
  let expanded = $state(false);
  let audioUrl = $state<string | null>(null);
  let audioEl = $state<HTMLAudioElement | null>(null);
  let playing = $state(false);
  let busy = $state(false);

  let displayedText = $derived.by(() => {
    if (variant === 0) {
      return rec.cleaned_text || rec.transcript || "(no transcript)";
    }
    return rec.transcript || "(no transcript)";
  });

  let hasBothVariants = $derived(!!rec.cleaned_text && !!rec.transcript && rec.cleaned_text !== rec.transcript);
  let isError = $derived(rec.status === "error");
  // Retry is now offered for every recording — failed ones (recover from
  // a transient API outage / network blip) and successful ones (the user
  // didn't like the transcript and wants a fresh STT pass). Disabled only
  // while a retry is in flight or while the recording is mid-flow.
  let retryDisabled = $derived(
    busy || rec.status === "recording" || rec.status === "transcribing" || rec.status === "cleaning",
  );

  // Inspector — (i) button. Shows: full error, retry count, providers
  // used, Clippy note, audio path. Surfaced for every recording so the
  // user can see what happened on success too. Red dot when there's an
  // error worth noticing.
  let showInspector = $state(false);
  let inspectorHasNews = $derived(isError || !!rec.error);

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

<article class="row" class:expanded class:error-row={isError}>
  <header class="row-head">
    <button class="row-toggle" onclick={() => (expanded = !expanded)} aria-label="Toggle expand">
      <svg class="caret" class:open={expanded} viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
        <path d="M 5 3 L 11 8 L 5 13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>
    <div class="meta">
      <span class="when">{timeShort(rec.created_at)}</span>
      <span class="dot">·</span>
      <span class="dur">{durationShort(rec.duration_ms)}</span>
      <span class="mode {rec.mode}">{rec.mode}</span>
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
        onclick={() => (showInspector = !showInspector)}
        aria-label="Show recording details and event log"
        aria-expanded={showInspector}
        title={inspectorHasNews ? "Details (error logged)" : "Details"}
      >
        i
      </button>
    </div>

    <div class="actions">
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

      <!-- Retry: always visible. Highlighted for errored rows since
           it's the obvious recovery action; a normal subtle button on
           successful rows since most users won't click it. -->
      <button
        class="action-btn retry"
        class:emphasized={isError}
        onclick={retry}
        disabled={retryDisabled}
        title={isError ? "Retry transcription" : "Re-run transcription on this audio"}
      >
        <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
          <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
        </svg>
      </button>
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
      <!-- Delete used to live in the action button row. Moved here so it
           takes a deliberate click instead of being a thumb-reachable
           danger button alongside Play/Copy/Retry. Still confirms. -->
      <div class="expanded-actions">
        <button class="delete-link" onclick={remove} disabled={busy}>Delete recording</button>
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

  {#if hasBothVariants}
    <!-- Compact inline variant toggle — minimal chevrons + tiny label.
         Sits flush against the transcript so it doesn't steal vertical space. -->
    <button
      class="variant-toggle"
      onclick={() => (variant = variant === 0 ? 1 : 0)}
      title="Switch between Polished (LLM-cleaned) and Base (raw transcript)"
    >
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
        <path d="M 10 3 L 5 8 L 10 13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span class="variant-name-compact">{variant === 0 ? "Polished" : "Base"}</span>
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
        <path d="M 6 3 L 11 8 L 6 13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </button>
  {/if}

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
  }

  .row:hover {
    background: var(--bg-subtle);
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
