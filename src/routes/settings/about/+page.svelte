<script lang="ts">
  // About — what you're running, what's available, and one button to move
  // between them.
  //
  // This replaces a "Check for updates" block that lived at the bottom of
  // App & data. Two problems with that: you had to know it was there and
  // scroll to it, and it reported ONE version — whichever GitHub listed first
  // — so it could not answer "what am I on, what's the newest stable, and is
  // there a newer nightly?", which are three separate questions.

  import { onMount, onDestroy } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, onUpdateProgress, type ReleaseInfo, type UpdateStatus, type UpdateProgress } from "$lib/api";

  let status = $state<UpdateStatus | null>(null);
  let checking = $state(false);
  let error = $state<string | null>(null);
  let progress = $state<UpdateProgress | null>(null);
  /** Tag currently being installed, so only that card shows a busy state. */
  let installing = $state<string | null>(null);
  let installError = $state<string | null>(null);
  let installNote = $state<string | null>(null);
  let unlisten: (() => void) | undefined;

  async function check() {
    checking = true;
    error = null;
    try {
      status = await api.updateStatus();
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }

  onMount(async () => {
    void check();
    unlisten = await onUpdateProgress((p) => (progress = p));
  });
  onDestroy(() => unlisten?.());

  async function install(rel: ReleaseInfo) {
    installing = rel.tag;
    installError = null;
    installNote = null;
    progress = null;
    try {
      const path = await api.downloadAndInstall(rel.tag);
      if (status?.can_self_install) {
        // Windows: the installer is up and the app is about to exit. Anything
        // rendered here is only on screen for a moment.
        installNote = "Installer launched — wispr-fox will close to finish updating.";
      } else {
        installNote = `Downloaded to ${path}. Drag wispr-fox to Applications to finish.`;
      }
    } catch (e) {
      installError = String(e);
      installing = null;
    }
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  function fmtDate(iso: string | null): string {
    if (!iso) return "";
    const t = Date.parse(iso);
    if (Number.isNaN(t)) return "";
    return new Date(t).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });
  }

  let pct = $derived(
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : 0,
  );

  // "Up to date" has to mean up to date on the channel you are actually on.
  // A user running a nightly is not behind just because a stable exists with a
  // lower version number.
  let onLatest = $derived(
    !!status && !status.stable?.newer && !status.nightly?.newer,
  );
</script>

<section>
  <h2>About</h2>
  <p class="lede">
    What you're running, what's available, and one button to move between them.
  </p>

  <div class="about-hero">
    <img class="about-fox" src="/fox/fox-success.png" alt="" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")} />
    <div class="about-id">
      <div class="about-name">wispr-fox</div>
      <div class="about-version">
        v{status?.current ?? "…"}
        {#if status?.current_is_nightly}
          <span class="chan chan-nightly">nightly</span>
        {:else if status}
          <span class="chan chan-stable">stable</span>
        {/if}
      </div>
      {#if status && onLatest}
        <div class="about-sub ok">You're on the latest build.</div>
      {:else if status}
        <div class="about-sub new">An update is available below.</div>
      {/if}
    </div>
    <button class="btn secondary" onclick={check} disabled={checking}>
      {checking ? "Checking…" : "Check again"}
    </button>
  </div>

  {#if error}
    <p class="err-line">⚠ Couldn't reach GitHub — {error}</p>
  {/if}

  {#snippet channel(rel: ReleaseInfo | null, kind: "stable" | "nightly")}
    <article class="rel-card" class:available={rel?.newer}>
      <div class="rel-head">
        <span class="rel-kind">{kind === "stable" ? "Latest stable" : "Latest nightly"}</span>
        {#if rel?.newer}<span class="rel-badge">Available</span>{/if}
      </div>

      {#if !rel}
        <div class="rel-ver muted">
          {kind === "stable" ? "None published yet" : "Nothing newer than stable"}
        </div>
        <p class="rel-note">
          {kind === "nightly"
            ? "Nightlies only show here when they're newer than the latest stable — otherwise installing one would move you backwards."
            : "No stable release found."}
        </p>
      {:else}
        <div class="rel-ver">
          {rel.tag}
          {#if fmtDate(rel.published_at)}<span class="rel-date"> · {fmtDate(rel.published_at)}</span>{/if}
        </div>
        {#if rel.summary}<p class="rel-note">{rel.summary}</p>{/if}

        <div class="rel-actions">
          {#if rel.newer && rel.asset}
            <button
              class="btn primary"
              onclick={() => install(rel)}
              disabled={installing !== null}
            >
              {#if installing === rel.tag}
                {progress?.phase === "launching" ? "Starting installer…" : `Downloading… ${pct}%`}
              {:else}
                {status?.can_self_install ? "Install" : "Download"} {rel.tag}
              {/if}
            </button>
          {:else if rel.newer && !rel.asset}
            <span class="rel-note warn">No installer for this platform in that release.</span>
          {:else}
            <span class="rel-note">You're on this or newer.</span>
          {/if}
          <button class="btn ghost" onclick={() => openUrl(rel.html_url)}>Release notes</button>
        </div>

        {#if installing === rel.tag && progress}
          <div class="prog" aria-label="Download progress">
            <div class="prog-fill" style="width: {pct}%"></div>
          </div>
          <div class="prog-text">
            {fmtBytes(progress.downloaded)} of {fmtBytes(progress.total)}
            {#if rel.asset} · {rel.asset.name}{/if}
          </div>
        {/if}
      {/if}
    </article>
  {/snippet}

  <div class="rel-grid">
    {@render channel(status?.stable ?? null, "stable")}
    {@render channel(status?.nightly ?? null, "nightly")}
  </div>

  {#if installError}
    <p class="err-line">⚠ {installError}</p>
  {/if}
  {#if installNote}
    <p class="ok-line">{installNote}</p>
  {/if}

  <p class="lede tight">
    {#if status?.can_self_install}
      Installing downloads the official installer from this project's GitHub
      releases and runs it. wispr-fox closes so the installer can replace files
      it is using, then reopens.
    {:else}
      Downloading fetches the official build from this project's GitHub
      releases and opens it. macOS builds aren't code-signed, so the last step
      — dragging wispr-fox to Applications — stays manual.
    {/if}
  </p>

  <h3>Nightly or stable?</h3>
  <p class="lede tight">
    <strong>Stable</strong> is the tested one and what you want by default.
    <strong>Nightly</strong> is newer and gets fixes and features first, but has
    had less real use. You can move between them freely in either direction —
    both install over the top of each other, and your history, settings and API
    keys are untouched by an update.
  </p>

  <h3>Links</h3>
  <div class="link-row">
    <button class="btn ghost" onclick={() => openUrl("https://github.com/kumaradarsh1993/wispr-fox/releases")}>All releases</button>
    <button class="btn ghost" onclick={() => openUrl("https://github.com/kumaradarsh1993/wispr-fox")}>Source</button>
    <button class="btn ghost" onclick={() => openUrl("https://github.com/kumaradarsh1993/wispr-fox/issues")}>Report an issue</button>
  </div>
</section>

<style>
  .about-hero {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 18px 20px;
    margin-bottom: 18px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xl, 14px);
    background:
      linear-gradient(110deg, color-mix(in srgb, var(--accent-fade) 70%, var(--bg-card)), var(--bg-card));
    width: min(100%, 920px);
  }
  .about-fox {
    width: 64px;
    height: 64px;
    object-fit: contain;
    flex-shrink: 0;
  }
  .about-id {
    flex: 1;
    min-width: 0;
  }
  .about-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .about-version {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .chan {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px 8px;
    border-radius: 999px;
  }
  .chan-stable {
    background: var(--success-fade, rgba(60, 150, 90, 0.15));
    color: var(--success, #3c965a);
  }
  .chan-nightly {
    background: var(--accent-fade);
    color: var(--accent);
  }
  .about-sub {
    font-size: 12px;
    margin-top: 3px;
  }
  .about-sub.ok { color: var(--text-secondary); }
  .about-sub.new { color: var(--accent); font-weight: 600; }

  .rel-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    width: min(100%, 920px);
  }
  @container settings (max-width: 760px) {
    .rel-grid { grid-template-columns: 1fr; }
  }

  .rel-card {
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    background: var(--bg-card);
    padding: 16px 18px;
  }
  .rel-card.available {
    border-color: color-mix(in srgb, var(--accent) 40%, var(--border-subtle));
    background: color-mix(in srgb, var(--accent-fade) 22%, var(--bg-card));
  }
  .rel-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 5px;
  }
  .rel-kind {
    font-size: 10.5px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }
  .rel-badge {
    font-size: 9.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #fff;
    background: var(--accent);
    padding: 2px 7px;
    border-radius: 999px;
  }
  .rel-ver {
    font-size: 16px;
    font-weight: 650;
    color: var(--text-primary);
  }
  .rel-ver.muted {
    color: var(--text-secondary);
    font-weight: 500;
    font-size: 14px;
  }
  .rel-date {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }
  .rel-note {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.45;
  }
  .rel-note.warn { color: var(--warning, #c47a30); }
  .rel-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
  }

  .prog {
    margin-top: 12px;
    height: 6px;
    border-radius: 999px;
    background: var(--bg-subtle);
    overflow: hidden;
  }
  .prog-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 999px;
    transition: width 120ms linear;
  }
  .prog-text {
    margin-top: 5px;
    font-size: 11px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .err-line {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--danger, #b3261e);
  }
  .ok-line {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--success, #3c965a);
  }

  .link-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .btn {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 7px 13px;
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 550;
    cursor: pointer;
    background: var(--bg-card);
    color: var(--text-primary);
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
  }
  .btn:hover:not(:disabled) { background: var(--bg-subtle); }
  .btn:disabled { opacity: 0.6; cursor: default; }
  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .btn.primary:hover:not(:disabled) { background: var(--accent-hover, var(--accent)); }
  .btn.ghost {
    background: transparent;
    color: var(--text-secondary);
  }
  .btn.ghost:hover { color: var(--text-primary); background: var(--bg-subtle); }
</style>
