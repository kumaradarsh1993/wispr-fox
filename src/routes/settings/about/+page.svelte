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
  import UpdatePanel from "$lib/UpdatePanel.svelte";

  const REPO = "https://github.com/kumaradarsh1993/wispr-fox";
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

  <h3>Links</h3>
  <div class="link-row">
    <button class="btn ghost" onclick={() => openUrl(`${REPO}/releases`)}>All releases</button>
    <button class="btn ghost" onclick={() => openUrl(REPO)}>Source</button>
    <button class="btn ghost" onclick={() => openUrl(`${REPO}/issues`)}>Report an issue</button>
  </div>
</section>

<style>
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
