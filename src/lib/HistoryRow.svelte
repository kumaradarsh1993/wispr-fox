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
  let canRetry = $derived(isError && rec.audio_path);

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
        <span class="err-pill">Failed — click ↻ to retry</span>
      {/if}
    </div>

    <div class="actions">
      <button class="action-btn play" onclick={togglePlay} disabled={busy} title="Play / pause audio">
        {#if playing}
          <svg viewBox="0 0 16 16" width="14" height="14"><rect x="4" y="3" width="3" height="10" fill="currentColor"/><rect x="9" y="3" width="3" height="10" fill="currentColor"/></svg>
        {:else}
          <svg viewBox="0 0 16 16" width="14" height="14"><path d="M 5 3 L 13 8 L 5 13 Z" fill="currentColor"/></svg>
        {/if}
      </button>

      {#if isError}
        <button class="action-btn retry" onclick={retry} disabled={busy} title="Retry transcription">
          <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
            <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>
          </svg>
        </button>
      {:else}
        <button class="action-btn" onclick={copyText} disabled={busy} title="Copy text">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <rect x="4" y="3" width="8" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.6"/>
            <rect x="2.5" y="1.5" width="8" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.4" opacity="0.55"/>
          </svg>
        </button>
      {/if}

      <button class="action-btn delete" onclick={remove} disabled={busy} title="Delete (audio + text)">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path d="M 3 4 L 13 4 M 5 4 L 5 13 A 1 1 0 0 0 6 14 L 10 14 A 1 1 0 0 0 11 13 L 11 4 M 6 4 L 6 2.5 A 0.5 0.5 0 0 1 6.5 2 L 9.5 2 A 0.5 0.5 0 0 1 10 2.5 L 10 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
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
  </div>

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

  .action-btn.retry {
    background: var(--warning-fade);
    border-color: var(--warning);
    color: var(--warning);
  }
  .action-btn.retry:hover:not(:disabled) {
    background: var(--warning);
    color: #fff;
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
