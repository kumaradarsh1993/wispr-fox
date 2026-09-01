<script lang="ts">
  // About — what you're running, what's available, and one button to move
  // between them.
  //
  // The two-channel picker, the download, the silent install and the relaunch
  // all live in `$lib/UpdatePanel.svelte`, which is byte-identical in wispr-fox,
  // FoxCull, Fox MD and Fox Mark. This page is the wispr-fox frame around it:
  // the fox, the version, and the links. Keeping the mechanism shared is the
  // point — an update bug fixed once is fixed in every app.

  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import UpdatePanel from "$lib/UpdatePanel.svelte";
  import { api, type PlatformDiagnostic } from "$lib/api";

  const REPO = "https://github.com/kumaradarsh1993/wispr-fox";

  // Diagnostics. Two of this app's hardest bugs — whether the floater is
  // pinned across macOS Spaces, and whether the running binary actually holds
  // Accessibility — are invisible from the Windows machine most of the code is
  // written on, and both have been misdiagnosed by reasoning about them from a
  // distance. These numbers are read back from the live NSWindow and from
  // macOS itself, so a bug report can carry facts instead of symptoms.
  let diag = $state<PlatformDiagnostic | null>(null);
  let diagErr = $state("");
  let copied = $state(false);

  async function runDiagnostic() {
    diagErr = "";
    copied = false;
    try {
      diag = await api.platformDiagnostic();
    } catch (e) {
      diagErr = String(e);
    }
  }

  function diagText(d: PlatformDiagnostic): string {
    const hex = (n: number | null) => (n === null ? "unknown" : `${n} (0x${n.toString(16)})`);
    return [
      `wispr-fox ${d.version} on ${d.os}`,
      `exe:    ${d.exe_path}`,
      `bundle: ${d.bundle_path ?? "(not in an .app bundle)"}`,
      `accessibility trusted: ${d.accessibility_trusted}`,
      `floater visible: ${d.floater_visible}`,
      `floater level: ${d.floater_level ?? "unknown"} (want 25)`,
      `floater collectionBehavior: ${hex(d.floater_collection_behavior)} (want bits 0 and 8)`,
      `floater pinned to all Spaces: ${d.floater_pinned}`,
    ].join("\n");
  }

  async function copyDiagnostic() {
    if (!diag) return;
    try {
      await writeText(diagText(diag));
      copied = true;
    } catch (e) {
      diagErr = String(e);
    }
  }
</script>

<section>
  <h2>About</h2>
  <p class="lede">
    What you're running, what's available, and one button to move between them.
  </p>

  <div class="about-hero">
    <img
      class="about-fox"
      src="/fox/fox-success.png"
      alt=""
      onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")}
    />
    <UpdatePanel title="wispr-fox" />
  </div>

  <h3>Diagnostics</h3>
  <p class="diag-lede">
    What this build actually is, and what the operating system thinks of it.
    Useful when something works on one machine and not another — paste it into
    a bug report rather than describing the symptom.
  </p>
  <div class="link-row">
    <button class="btn ghost" onclick={runDiagnostic}>Run diagnostic</button>
    {#if diag}
      <button class="btn ghost" onclick={copyDiagnostic}>{copied ? "Copied" : "Copy"}</button>
    {/if}
  </div>
  {#if diagErr}
    <p class="diag-err">{diagErr}</p>
  {/if}
  {#if diag}
    <pre class="diag">{diagText(diag)}</pre>
    {#if diag.os === "macos" && !diag.floater_pinned}
      <p class="diag-err">
        The avatar is not pinned across desktops. Level should be 25 and
        collectionBehavior should have bits 0 and 8 set.
      </p>
    {/if}
  {/if}

  <h3>Links</h3>
  <div class="link-row">
    <button class="btn ghost" onclick={() => openUrl(`${REPO}/releases`)}>All releases</button>
    <button class="btn ghost" onclick={() => openUrl(REPO)}>Source</button>
    <button class="btn ghost" onclick={() => openUrl(`${REPO}/issues`)}>Report an issue</button>
  </div>
</section>

<style>
  .diag-lede {
    width: min(100%, 920px);
    margin: 0 0 10px;
    opacity: 0.8;
  }
  .diag {
    width: min(100%, 920px);
    overflow-x: auto;
    margin: 12px 0 0;
    padding: 12px 14px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg, 10px);
    background: var(--bg-card);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.55;
    white-space: pre;
  }
  .diag-err {
    width: min(100%, 920px);
    margin: 10px 0 0;
    color: var(--danger, #b3261e);
  }
  /* Full-bleed section, width carried by the inner wrapper — the pattern the
     rest of Settings uses, so the scrollbar stays at the pane edge rather than
     halfway across the page. */
  .about-hero {
    display: flex;
    align-items: flex-start;
    gap: 18px;
    padding: 18px 20px;
    margin-bottom: 18px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-xl, 14px);
    background: linear-gradient(
      110deg,
      color-mix(in srgb, var(--accent-fade) 70%, var(--bg-card)),
      var(--bg-card)
    );
    width: min(100%, 920px);
  }
  .about-fox {
    width: 64px;
    height: 64px;
    object-fit: contain;
    flex-shrink: 0;
  }
  /* The panel is the flexible half of the hero and manages its own layout. */
  .about-hero :global(.fxu) {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 18px;
  }
  h3 {
    margin: 20px 0 8px;
    font-size: 14px;
  }
  .lede {
    margin: 0 0 16px;
    color: var(--text-secondary);
    font-size: 13px;
    max-width: 68ch;
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
  .btn.ghost {
    background: transparent;
    color: var(--text-secondary);
  }
  .btn.ghost:hover { color: var(--text-primary); background: var(--bg-subtle); }
</style>
