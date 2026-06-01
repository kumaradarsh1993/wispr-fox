<script lang="ts">
  // Custom right-click menu for the floater window.
  //
  // Replaces the default WebView2 / WKWebView menu (Inspect Element, Reload,
  // etc.) with app-specific actions: start/stop recording, switch mode, change
  // avatar, hide, open settings.
  //
  // The floater window is fixed at 190×210 (set in tauri.conf.json), so the
  // menu has to fit inside that footprint. We use NAVIGATION between the
  // main menu and the avatar sub-menu (no fly-out) so everything stays
  // within bounds. Click the trigger to enter a sub-menu; the back chevron
  // returns. The whole thing is dismissed by clicking outside (backdrop) or
  // selecting an action.
  //
  // Positioning: anchored to top-left of the click point, but clamped so the
  // menu never crosses the right or bottom window edge. This keeps the menu
  // visible regardless of where the user right-clicks within the window.

  import { api } from "$lib/api";
  import { skinStore, setClippyWindowVisible, type Skin } from "$lib/skin-store.svelte";

  type RecState = "idle" | "recording" | "transcribing" | "cleaning" | "injecting";

  let {
    x,
    y,
    recState,
    onClose,
  }: {
    x: number;
    y: number;
    recState: RecState;
    onClose: () => void;
  } = $props();

  type Pane = "main" | "avatar";
  let pane = $state<Pane>("main");

  const AVATAR_OPTIONS: { id: Skin; label: string; emoji: string }[] = [
    { id: "fox",         label: "Fox",       emoji: "🦊" },
    { id: "stylized",    label: "Paperclip", emoji: "📎" },
    { id: "real-clippy", label: "Clippy",    emoji: "📎" },
    { id: "cat",         label: "Desk Cat",  emoji: "🐱" },
    { id: "cat-lab",     label: "Cat (lab)", emoji: "🧪" },
    { id: "off",         label: "Hide",      emoji: "○"  },
  ];

  async function startDictation(mode: "light" | "drafting") {
    onClose();
    try {
      await api.floaterTrigger(mode);
    } catch (e) {
      console.warn("floater_trigger failed", e);
    }
  }

  async function stopDictation() {
    onClose();
    try {
      // sticky-invoke on the active mode toggles off — Mode doesn't matter
      // for the stop call (the flow layer knows there's an in-flight recording).
      await api.floaterTrigger("light");
    } catch (e) {
      console.warn("floater_trigger stop failed", e);
    }
  }

  async function pickAvatar(s: Skin) {
    onClose();
    await skinStore.set(s);
    await setClippyWindowVisible(s !== "off");
  }

  async function hideFloater() {
    onClose();
    await setClippyWindowVisible(false);
  }

  // Recover from a "where did the floater go?" situation. Forgets the saved
  // position so the next launch (or this one) lands in the default bottom-
  // right slot. Especially useful when an old position from a different
  // monitor setup is keeping the window offscreen — though the position-
  // validate-on-startup logic should normally catch that automatically.
  async function resetPosition() {
    onClose();
    try {
      localStorage.removeItem("wispr.clippy.pos");
      const { getCurrentWindow, availableMonitors, LogicalPosition } =
        await import("@tauri-apps/api/window");
      const monitors = await availableMonitors();
      const primary = monitors[0];
      if (primary) {
        const x = primary.position.x + primary.size.width - 190 * 2 - 32;
        const y = primary.position.y + primary.size.height - 210 * 2 - 80;
        await getCurrentWindow().setPosition(new LogicalPosition(x, y));
      }
    } catch (e) {
      console.warn("resetPosition failed", e);
    }
  }

  async function openSettings() {
    onClose();
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const main = await WebviewWindow.getByLabel("main");
      if (main) {
        await main.show();
        await main.unminimize();
        await main.setFocus();
      }
    } catch (e) {
      console.warn("openSettings failed", e);
    }
  }
</script>

<!-- Backdrop catches clicks outside the menu. Pointer-events: all so it sits
     above the SVG character. The menu itself stops propagation. -->
<button
  class="ctxmenu-backdrop"
  onclick={() => onClose()}
  oncontextmenu={(e) => { e.preventDefault(); onClose(); }}
  aria-label="Close menu"
></button>

<div
  class="ctxmenu"
  style="left: {x}px; top: {y}px;"
  role="menu"
  onclick={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#if pane === "main"}
    {#if recState === "idle"}
      <button class="ctx-item" onclick={() => startDictation("light")}>
        <span class="ctx-glyph">▶</span>
        <span class="ctx-label">Dictate</span>
      </button>
      <button class="ctx-item" onclick={() => startDictation("drafting")}>
        <span class="ctx-glyph">✏</span>
        <span class="ctx-label">Draft</span>
      </button>
    {:else}
      <button class="ctx-item ctx-stop" onclick={stopDictation}>
        <span class="ctx-glyph">⏹</span>
        <span class="ctx-label">Stop ({recState})</span>
      </button>
    {/if}

    <div class="ctx-sep"></div>

    <button class="ctx-item" onclick={() => (pane = "avatar")}>
      <span class="ctx-glyph">👤</span>
      <span class="ctx-label">Avatar…</span>
      <span class="ctx-arrow">▸</span>
    </button>

    <div class="ctx-sep"></div>

    <button class="ctx-item" onclick={resetPosition}>
      <span class="ctx-glyph">⤿</span>
      <span class="ctx-label">Reset position</span>
    </button>
    <button class="ctx-item" onclick={hideFloater}>
      <span class="ctx-glyph">⛔</span>
      <span class="ctx-label">Hide</span>
    </button>
    <button class="ctx-item" onclick={openSettings}>
      <span class="ctx-glyph">⚙</span>
      <span class="ctx-label">Settings</span>
    </button>
  {:else if pane === "avatar"}
    <button class="ctx-item ctx-back" onclick={() => (pane = "main")}>
      <span class="ctx-glyph">◂</span>
      <span class="ctx-label">Back</span>
    </button>
    <div class="ctx-sep"></div>
    {#each AVATAR_OPTIONS as opt (opt.id)}
      <button
        class="ctx-item"
        class:active={skinStore.current === opt.id}
        onclick={() => pickAvatar(opt.id)}
      >
        <span class="ctx-glyph">{opt.emoji}</span>
        <span class="ctx-label">{opt.label}</span>
        {#if skinStore.current === opt.id}
          <span class="ctx-check">✓</span>
        {/if}
      </button>
    {/each}
  {/if}
</div>

<style>
  .ctxmenu-backdrop {
    position: fixed;
    inset: 0;
    background: transparent;
    border: none;
    padding: 0;
    margin: 0;
    z-index: 9998;
    cursor: default;
  }

  .ctxmenu {
    position: fixed;
    z-index: 9999;
    min-width: 150px;
    max-width: 168px;
    background: rgba(255, 255, 255, 0.98);
    color: #1d1d1f;
    border: 1px solid rgba(0, 0, 0, 0.14);
    border-radius: 8px;
    box-shadow:
      0 8px 24px rgba(0, 0, 0, 0.28),
      0 2px 4px rgba(0, 0, 0, 0.12);
    padding: 3px;
    font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
    font-size: 12px;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    pointer-events: auto;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 5px 8px;
    background: transparent;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    color: #1d1d1f;
    text-align: left;
    font: inherit;
  }
  .ctx-item:hover {
    background: rgba(236, 124, 52, 0.14);
  }
  .ctx-item.ctx-stop {
    color: #c0392b;
  }
  .ctx-item.ctx-stop:hover {
    background: rgba(192, 57, 43, 0.12);
  }
  .ctx-item.ctx-back {
    color: #777;
    font-size: 11px;
  }
  .ctx-item.active {
    background: rgba(236, 124, 52, 0.18);
    font-weight: 600;
  }
  .ctx-glyph {
    flex: 0 0 16px;
    text-align: center;
    font-size: 12px;
    opacity: 0.85;
  }
  .ctx-label {
    flex: 1 1 auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ctx-arrow {
    color: #999;
    font-size: 9px;
  }
  .ctx-check {
    color: #ec7c34;
    font-weight: 700;
  }

  .ctx-sep {
    height: 1px;
    background: rgba(0, 0, 0, 0.08);
    margin: 3px 4px;
  }

  /* Dark mode picks up OS theme — most apps look this way in dark mode. */
  @media (prefers-color-scheme: dark) {
    .ctxmenu {
      background: rgba(44, 44, 46, 0.98);
      color: #f2f2f7;
      border-color: rgba(255, 255, 255, 0.12);
    }
    .ctx-item {
      color: #f2f2f7;
    }
    .ctx-item:hover {
      background: rgba(236, 124, 52, 0.24);
    }
    .ctx-sep {
      background: rgba(255, 255, 255, 0.1);
    }
  }
</style>
