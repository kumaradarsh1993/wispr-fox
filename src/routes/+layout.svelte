<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AppContextMenu from "$lib/AppContextMenu.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { skinStore, type Skin } from "$lib/skin-store.svelte";
  import {
    avatarVisibility,
    applyVisibilityWindow,
    type AvatarVisibility,
  } from "$lib/avatar-visibility.svelte";
  import { settings } from "$lib/settings-store.svelte";
  import {
    api,
    type InputDeviceInfo,
  } from "$lib/api";
  import { account } from "$lib/account-store.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";
  import { avatarLabel } from "$lib/avatar-catalog";
  import { prettyHotkey } from "$lib/hotkey-display";
  import { STT_PROVIDERS } from "$lib/provider-options";

  let { children } = $props();

  let collapsed = $state(false);
  let sidebarWidth = $state(272);
  let flowBusy = $state(false);
  let resizingSidebar = $state(false);
  let appApiPromise: Promise<typeof import("@tauri-apps/api/app")> | null = null;

  // macOS uses an overlay titlebar (tauri.macos.conf.json): the webview runs
  // under the traffic lights, so the sidebar top needs a ~36px inset and a
  // drag region. Windows/Linux keep native decorations and no inset.
  const isMacShell =
    typeof navigator !== "undefined" && /Mac/.test(navigator.platform ?? "");

  function loadAppApi() {
    appApiPromise ??= import("@tauri-apps/api/app");
    return appApiPromise;
  }

  // macOS auto-paste needs Accessibility permission (CGEvent injection + the
  // Cmd+V fallback both require it). `accessibility_ok` returns true on
  // Windows/Linux, so this banner only ever appears on a Mac that hasn't
  // granted it yet. Starts assumed-OK so it never flashes before the check
  // resolves or on non-Mac platforms.
  let accessibilityOk = $state(true);
  let a11yDismissed = $state(false);
  let showA11yBanner = $derived(!accessibilityOk && !a11yDismissed);

  async function checkAccessibility() {
    try {
      accessibilityOk = await api.accessibilityOk();
      if (accessibilityOk) rememberGranted();
      else {
        try {
          a11yHadItBefore = localStorage.getItem(A11Y_GRANTED_BEFORE) === "1";
        } catch {}
      }
    } catch (e) {
      console.warn("accessibility check failed", e);
      accessibilityOk = true; // fail open — never nag if the check itself errors
    }
  }
  // The repair path. Toggling the System Settings switch does not rebind a
  // grant whose entry points at a previous build's signature — the entry has to
  // be removed. `repair_accessibility` does that and re-prompts, and the answer
  // is almost always "still not trusted", because macOS hands a fresh grant to
  // the process only when it next starts. So the honest thing to report is
  // "granted — now relaunch", not a failure.
  let a11yRepairNote = $state("");
  let a11yWhy = $state(false);

  // Fresh install or an update that dropped the grant? The two need different
  // words: "wispr-fox needs permission" is confusing to someone who granted it
  // last week, and "macOS dropped it during the update" is meaningless to
  // someone who just installed. One marker distinguishes them — set the first
  // time we ever see the permission held, and never cleared.
  const A11Y_GRANTED_BEFORE = "wispr.a11y.grantedBefore";
  let a11yHadItBefore = $state(false);
  let a11yHeadline = $derived(
    a11yHadItBefore
      ? "macOS dropped the Accessibility permission during the update."
      : "wispr-fox needs Accessibility permission to paste into other apps.",
  );
  let a11yAction = $derived(a11yHadItBefore ? "Repair" : "Grant");

  function rememberGranted() {
    try {
      localStorage.setItem(A11Y_GRANTED_BEFORE, "1");
    } catch {}
  }

  async function repairAccessibility() {
    a11yRepairNote = "Asking macOS…";
    try {
      const trusted = await api.repairAccessibility();
      accessibilityOk = trusted;
      if (trusted) {
        rememberGranted();
        a11yRepairNote = "Done.";
      } else {
        a11yRepairNote = "Approve it in the macOS dialog, then quit and reopen wispr-fox.";
      }
    } catch (e) {
      console.warn("accessibility repair failed", e);
      a11yRepairNote =
        "Couldn't do it automatically — in Accessibility, select wispr-fox, press −, then add it with +.";
    }
  }
  async function grantAccessibility() {
    try {
      await api.openAccessibilitySettings();
    } catch (e) {
      console.warn("open accessibility settings failed", e);
    }
  }

  // Reactive theme application — sets document.body[data-theme] whenever the
  // settings.theme value changes. Valid values: "auto" | "light" | "dark" | "retro".
  $effect(() => {
    const t = settings.s.theme || "auto";
    if (typeof document !== "undefined") {
      document.body.setAttribute("data-theme", t);
    }
    const nativeTheme = t === "dark" ? "dark" : t === "light" || t === "retro" ? "light" : null;
    loadAppApi()
      .then(({ setTheme }) => setTheme(nativeTheme))
      .catch((e) => console.warn("native theme sync failed", e));
  });

  // Avatar visibility tri-state ("Always show" / "While dictating" / "Hidden").
  // The single source of truth for whether the floater is on screen — decoupled
  // from the skin. Picking a skin never changes this.
  const VISIBILITY_OPTIONS: { id: AvatarVisibility; short: string; label: string }[] = [
    { id: "always", short: "On",   label: "Always show" },
    { id: "auto",   short: "Auto", label: "While dictating" },
    { id: "hidden", short: "Off",  label: "Hidden" },
  ];
  async function pickVisibility(v: AvatarVisibility) {
    await avatarVisibility.set(v);
    await applyVisibilityWindow(v);
  }

  // ── Quick mic picker ─────────────────────────────────────────────────────
  // Deliberately lists ONLY devices that are present right now: a mic that is
  // switched off shouldn't look selectable. The saved-but-absent case gets its
  // own explicit row instead of silently showing something else, because
  // "which mic am I actually on?" is the whole reason this is in the sidebar.
  //
  // Kept structurally independent of the rest of the sidebar (one block, one
  // derived value, no shared layout) so it can be pulled out cleanly if it
  // turns out to be clutter in daily use.
  let inputDevices = $state<InputDeviceInfo[]>([]);
  let currentMic = $derived(settings.s.input_device ?? "");
  let micMissing = $derived(
    Boolean(currentMic) && inputDevices.length > 0 && !inputDevices.some((d) => d.name === currentMic),
  );

  /** Trim the OS's decoration so the sidebar doesn't need 300px of width.
   *  "Headset (DJI MIC2 Hands-Free AG Audio)" → "DJI MIC2 Hands-Free AG Audio" */
  function shortMic(name: string): string {
    const inner = name.match(/\(([^)]+)\)\s*$/);
    return (inner ? inner[1] : name).trim();
  }

  async function refreshInputDevices() {
    try {
      inputDevices = await api.listInputDevices();
    } catch (e) {
      console.warn("sidebar mic list failed", e);
      inputDevices = [];
    }
  }

  async function changeMic(name: string) {
    await settings.set("input_device", name || null);
  }

  // Persist sidebar collapsed state across launches.
  onMount(() => {
    const shellV2 = localStorage.getItem("wispr.shell.field-v1") === "1";
    const saved = localStorage.getItem("wispr.sidebar.collapsed");
    const savedWidth = Number(localStorage.getItem("wispr.sidebar.width"));
    if (shellV2) {
      if (saved === "1") collapsed = true;
      if (Number.isFinite(savedWidth)) sidebarWidth = clampSidebarWidth(savedWidth);
    } else {
      // One-time shell migration: the old 320px settings-heavy rail is now a
      // calmer navigation + quick-controls surface. Start it at its designed
      // width once, then respect every user resize after that.
      sidebarWidth = 272;
      collapsed = false;
      localStorage.setItem("wispr.sidebar.width", "272");
      localStorage.setItem("wispr.sidebar.collapsed", "0");
      localStorage.setItem("wispr.shell.field-v1", "1");
    }
    skinStore.subscribe();
    avatarVisibility.subscribe();
    // "Always" must mean always. Rust shows the floater once at launch; if
    // anything hid it afterwards (a mis-click on a visibility menu, a Space
    // change during startup, a hidden→always flip that raced the window),
    // re-assert from the persisted value. Idempotent — show_floater on an
    // already-visible window is a no-op that also re-pins it to all Spaces.
    if (avatarVisibility.current === "always") {
      applyVisibilityWindow("always").catch(() => {});
    }

    // Init settings, then decide whether to show the main window. The
    // window starts hidden (tauri.conf.json visible=false) so we don't
    // flash it on screen if the user wants silent startup. After settings
    // load, show it ONLY if "open_silently" is off.
    (async () => {
      await settings.init();
      await refreshInputDevices();
      if (!settings.s.open_silently) {
        try {
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          await getCurrentWindow().show();
          await getCurrentWindow().setFocus();
        } catch (e) {
          console.warn("show-main on startup failed", e);
        }
      }
    })();

    // Start the account store here rather than leaving it to whichever page
    // happens to mount first. Its listeners have to be live before the Rust
    // side finishes the launch-time session restore, or the resulting
    // `wispr:auth_status` event lands with nobody subscribed and the app keeps
    // showing the pre-restore (signed-out) answer.
    account.init();

    // Tray menu can request navigation via wispr:navigate event.
    let unlisten: (() => void) | undefined;
    let unlistenFlow: (() => void) | undefined;
    listen<string>("wispr:navigate", (e) => {
      goto(e.payload);
    }).then((u) => (unlisten = u));
    listen<string>("wispr:state", (e) => {
      flowBusy = e.payload !== "idle";
    }).then((u) => (unlistenFlow = u));
    return () => {
      unlisten?.();
      unlistenFlow?.();
    };
  });

  // Accessibility-permission check (macOS auto-paste). Re-check on window
  // focus so the banner clears the moment the user grants it and tabs back.
  onMount(() => {
    checkAccessibility();
    const onFocus = () => checkAccessibility();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  function toggleSidebar() {
    collapsed = !collapsed;
    localStorage.setItem("wispr.sidebar.collapsed", collapsed ? "1" : "0");
  }

  function clampSidebarWidth(width: number): number {
    return Math.min(340, Math.max(236, Math.round(width)));
  }

  function setSidebarWidth(width: number) {
    sidebarWidth = clampSidebarWidth(width);
    localStorage.setItem("wispr.sidebar.width", String(sidebarWidth));
  }

  function startSidebarResize(e: PointerEvent) {
    e.preventDefault();
    collapsed = false;
    localStorage.setItem("wispr.sidebar.collapsed", "0");
    resizingSidebar = true;
    setSidebarWidth(e.clientX);
    const onMove = (ev: PointerEvent) => setSidebarWidth(ev.clientX);
    const onUp = () => {
      resizingSidebar = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function resizeSidebarWithKeyboard(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggleSidebar();
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      setSidebarWidth(sidebarWidth - 12);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      collapsed = false;
      localStorage.setItem("wispr.sidebar.collapsed", "0");
      setSidebarWidth(sidebarWidth + 12);
    }
  }

  // Hide chrome on /onboarding (full-bleed) and /clippy (floating window).
  let hideChrome = $derived(
    page.url?.pathname?.startsWith("/onboarding") ||
    page.url?.pathname?.startsWith("/clippy") ||
    false,
  );

  // The floater runs in its own window and owns its right-click menu
  // (FloaterContextMenu: skin, scale, position). Mounting the app-wide one
  // there too would put two handlers on the same event.
  let isFloater = $derived(page.url?.pathname?.startsWith("/clippy") ?? false);

  // Nav icons are inline stroke SVGs (see the snippet in the markup) instead
  // of emoji — emoji glyphs render with the OS emoji font (inconsistent
  // weight/colour, can't follow the theme), while currentColor strokes pick
  // up the active/hover accent automatically.
  type NavItem = { href: string; label: string; icon: "history" | "stats" | "settings" };
  const navItems: NavItem[] = [
    { href: "/history", label: "Home", icon: "history" },
    { href: "/stats", label: "Insights", icon: "stats" },
    { href: "/settings", label: "Settings", icon: "settings" },
  ];

  // Hotkey reminder rendered at the top of the sidebar — always visible.
  // Delegates to prettyHotkey() so the symbols match the user's platform:
  // "Ctrl+Alt+D" → "⌃⌥D" on Mac, "Ctrl+Alt+D" on Windows; "Super+F8" →
  // "⌘F8" on Mac, "Win+F8" on Windows.
  const shortcutDisplay = prettyHotkey;

  function isActive(href: string): boolean {
    const path = page.url?.pathname ?? "/";
    if (href === "/") return path === "/";
    return path.startsWith(href);
  }

  let sidebarStyle = $derived(collapsed ? "" : `width: ${sidebarWidth}px;`);
</script>

<!-- App-wide right-click handling. Outside the chrome branch so onboarding
     gets it too: WebView2's Back / Reload / Save as / Print / Inspect menu
     should never appear on any surface of this app. -->
{#if !isFloater}
  <AppContextMenu />
{/if}

{#if hideChrome}
  {@render children?.()}
{:else}
  <div class="app-shell" data-mac={isMacShell || undefined}>
    <aside class="sidebar" class:collapsed class:resizing={resizingSidebar} style={sidebarStyle}>
      {#if isMacShell}
        <!-- Empty strip beside the traffic lights so the user can drag the
             window by its top edge. Deliberately NOT `data-tauri-drag-region`:
             Tauri maps a double-click on a drag region to MAXIMIZE, and with
             the overlay titlebar (no visible title bar) an accidentally
             maximized window reads as "the app opened full screen". Dragging
             via startDragging() gives the move behaviour without the zoom. -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="titlebar-drag"
          onmousedown={async (e) => {
            if (e.button !== 0) return;
            try {
              const { getCurrentWindow } = await import("@tauri-apps/api/window");
              await getCurrentWindow().startDragging();
            } catch {}
          }}
        ></div>
      {/if}
      <div class="sidebar-top">
        <!-- Universal sidebar-toggle icon (à la Claude/ChatGPT) — clearer
             affordance than the paperclip emoji previously used. -->
        <button class="brand" onclick={toggleSidebar} title={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
          <!-- Fox favicon as the brand mark — replaces the earlier abstract
               sidebar-toggle glyph. Same click handler (collapses/expands)
               but now the icon also carries the wispr-FOX identity. The
               bold flat fox face matches the design playbook reference
               far better than the inline SVG placeholder did. -->
          <span class="brand-mark">
            <img src="/fox/fox-favicon.png" alt="" />
          </span>
          {#if !collapsed}
            <span class="brand-text">wispr-fox</span>
          {/if}
        </button>

        {#snippet navIcon(icon: NavItem["icon"])}
          {#if icon === "history"}
            <!-- clock -->
            <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
              <circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-width="1.6" />
              <path d="M 8 4.8 V 8 L 10.4 9.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          {:else if icon === "stats"}
            <!-- bar chart -->
            <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
              <path d="M 3.2 13 V 9.5" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
              <path d="M 8 13 V 3.5" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
              <path d="M 12.8 13 V 6.5" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" />
            </svg>
          {:else}
            <!-- settings sliders -->
            <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
              <path d="M 2.5 5 H 13.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              <path d="M 2.5 11 H 13.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              <circle cx="6" cy="5" r="1.9" fill="var(--bg-sidebar)" stroke="currentColor" stroke-width="1.6" />
              <circle cx="10" cy="11" r="1.9" fill="var(--bg-sidebar)" stroke="currentColor" stroke-width="1.6" />
            </svg>
          {/if}
        {/snippet}

        <nav class="nav">
          {#each navItems as item (item.href)}
            <a href={item.href} class="nav-item" class:active={isActive(item.href)}>
              <span class="nav-icon">{@render navIcon(item.icon)}</span>
              {#if !collapsed}<span class="nav-label">{item.label}</span>{/if}
            </a>
          {/each}
        </nav>

        {#if !collapsed}
          <div class="hotkey-reminder">
            <div class="hk-title">Dictation keys</div>
            <div class="hk-row">
              <span class="hk-mode">Transcribe</span>
              <kbd>{shortcutDisplay(settings.s.light_hotkey)}</kbd>
            </div>
            <div class="hk-row">
              <span class="hk-mode">Draft</span>
              <kbd>{shortcutDisplay(settings.s.drafting_hotkey)}</kbd>
            </div>
            <div class="hk-row hk-row-tip">
              <span class="hk-mode">Stop</span>
              <kbd>Esc</kbd>
            </div>
          </div>
        {/if}

        {#if !collapsed}
          <section class="quick-card" aria-label="Quick controls">
            <div class="quick-head">
              <div>
                <span class="quick-kicker">Ready to write</span>
                <strong>Quick controls</strong>
              </div>
              <a href="/settings/dictation">All settings</a>
            </div>

            <div class="quick-row">
              <div class="quick-row-copy">
                <span class="quick-label">Listening with</span>
                <strong>{STT_PROVIDERS.find((p) => p.id === settings.s.stt_provider)?.label ?? settings.s.stt_provider}</strong>
              </div>
              <a class="quick-change" href="/settings/providers">Change</a>
            </div>

            <label class="clean-switch" title="Polish filler words and punctuation after Transcribe">
              <span>
                <strong>Polish Transcribe</strong>
                <small>{settings.s.auto_clean_in_light ? "On — uses your writing engine" : "Off — keeps the raw transcript"}</small>
              </span>
              <input
                type="checkbox"
                checked={settings.s.auto_clean_in_light}
                disabled={flowBusy}
                onchange={(e) => settings.set("auto_clean_in_light", (e.currentTarget as HTMLInputElement).checked)}
              />
            </label>

            <label class="quick-mic">
              <span class="quick-label">Microphone</span>
              <select
                aria-label="Microphone"
                value={currentMic}
                disabled={flowBusy}
                onfocus={refreshInputDevices}
                onchange={(e) => changeMic((e.currentTarget as HTMLSelectElement).value)}
              >
                <option value="">System default</option>
                {#each inputDevices as d (d.name)}
                  <option value={d.name}>{shortMic(d.name)}</option>
                {/each}
                {#if micMissing}
                  <option value={currentMic}>{shortMic(currentMic)} — not connected</option>
                {/if}
              </select>
              {#if micMissing}<small class="mic-note">Using system default until it reconnects.</small>{/if}
            </label>

            <div class="companion-quick">
              <a class="companion-link" href="/settings/appearance" title="Choose your avatar">
                <span class="companion-icon"><SkinIcon skin={skinStore.current} size={26} /></span>
                <span>
                  <small>Avatar</small>
                  <strong>{avatarLabel(skinStore.current)}</strong>
                </span>
              </a>
              <div class="vis-row" role="group" aria-label="Avatar visibility">
                {#each VISIBILITY_OPTIONS as v (v.id)}
                  <button
                    class="vis-btn"
                    class:active={avatarVisibility.current === v.id}
                    onclick={() => pickVisibility(v.id)}
                    title={v.label}
                    aria-label={v.label}
                  >{v.short}</button>
                {/each}
              </div>
            </div>
          </section>
        {/if}

        <!-- Replay onboarding — a quiet footer link so testers (and curious
             users) can re-walk the 3-screen flow without touching their keys. -->
        {#if !collapsed}
          <a class="replay-onboarding" href="/onboarding" data-sveltekit-preload-data="off">
            ↻ Replay onboarding
          </a>
        {/if}

        {#if !collapsed}
          <div class="sidebar-fox" aria-hidden="true">
            <img src="/fox/fox-hero.png" alt="" />
          </div>
        {/if}
      </div>

      <button
        type="button"
        class="sidebar-resizer"
        aria-label="Resize sidebar"
        onpointerdown={startSidebarResize}
        onkeydown={resizeSidebarWithKeyboard}
        ondblclick={toggleSidebar}
      ></button>

      <div class="sidebar-bottom">
        <!-- Account chip. The sidebar's only job besides nav: who am I, and
             one click to sign in / manage sync. -->
        <a
          class="account-chip"
          href="/settings/account"
          class:active={isActive("/settings/account")}
          title={account.signedIn ? account.status.email ?? "Account" : "Sign in to sync across devices"}
        >
          <span class="account-avatar" aria-hidden="true">
            {#if account.signedIn}
              {(account.status.email ?? "?").slice(0, 1).toUpperCase()}
            {:else}
              <svg viewBox="0 0 16 16" width="14" height="14"><circle cx="8" cy="5.5" r="3" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M2.5 14c.8-3 3-4.5 5.5-4.5s4.7 1.5 5.5 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
            {/if}
          </span>
          {#if !collapsed}
            <span class="account-text">
              {#if account.signedIn}
                <strong>{account.status.email}</strong>
                <small>{account.sync.state === "syncing" ? "Syncing…" : account.sync.state === "error" ? "Sync problem" : "Synced"}</small>
              {:else}
                <strong>Sign in</strong>
                <small>Sync notes across devices</small>
              {/if}
            </span>
          {/if}
        </a>
      </div>
    </aside>

    <main class="main-content">
      {#if showA11yBanner}
        <div class="a11y-banner" role="alert">
          <div class="a11y-row">
            <span class="a11y-text">{a11yHeadline}</span>
            <button class="a11y-btn" onclick={repairAccessibility}>{a11yAction}</button>
            <button
              class="a11y-link"
              onclick={() => (a11yWhy = !a11yWhy)}
              aria-expanded={a11yWhy}>Why?</button>
            <button class="a11y-x" onclick={() => (a11yDismissed = true)} aria-label="Dismiss">✕</button>
          </div>
          {#if a11yRepairNote}
            <p class="a11y-status">{a11yRepairNote}</p>
          {/if}
          {#if a11yWhy}
            <p class="a11y-why">
              macOS ties this permission to the app's signature, so it can look switched on in
              System Settings while pointing at an older build. {a11yAction} clears that and asks
              again. Until then your words go to the clipboard — press ⌘V.
              <button class="a11y-link" onclick={grantAccessibility}>Open System Settings</button>
              <button class="a11y-link" onclick={() => checkAccessibility()}>Re-check</button>
            </p>
          {/if}
        </div>
      {/if}
      {@render children?.()}
    </main>
  </div>
{/if}

<style>
  :global(body) {
    overflow: hidden;
  }

  .app-shell {
    display: grid;
    grid-template-columns: auto 1fr;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .sidebar {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 272px;
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--bg-sidebar) 96%, transparent), var(--bg-sidebar)),
      url('/fox/texture-paper.png');
    background-size: auto, 280px 280px;
    border-right: 1px solid var(--border);
    transition: width var(--motion-base) var(--ease-standard),
                background 200ms ease,
                border-color 200ms ease;
    overflow: hidden;
    color: var(--text-primary);
  }

  .sidebar.collapsed {
    width: 64px;
  }
  /* Traffic lights span ~70px from the left edge; a 64px collapsed rail
     would let page content slide under them. */
  .app-shell[data-mac] .sidebar.collapsed {
    width: 78px;
  }

  .sidebar.resizing {
    user-select: none;
  }

  .sidebar-resizer {
    position: absolute;
    top: 0;
    right: -3px;
    width: 6px;
    height: 100%;
    cursor: col-resize;
    z-index: 20;
    border: 0;
    background: transparent;
    padding: 0;
  }

  .sidebar-resizer::after {
    content: "";
    position: absolute;
    top: 0;
    right: 2px;
    width: 1px;
    height: 100%;
    background: transparent;
    transition: background 120ms ease, box-shadow 120ms ease;
  }

  .sidebar-resizer:hover::after,
  .sidebar-resizer:focus-visible::after,
  .sidebar.resizing .sidebar-resizer::after {
    background: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-fade);
  }

  /* macOS overlay titlebar: the traffic lights sit at (12px, ~13px) and are
     ~54px wide. A 36px strip keeps the brand row clear of them and doubles
     as the window's drag handle. Not rendered on other platforms. */
  .titlebar-drag {
    flex: none;
    height: 36px;
  }
  .app-shell[data-mac] .sidebar-top {
    padding-top: 4px;
  }

  .sidebar-top {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 16px 12px 12px;
    min-height: 0;
    /* When window is short, sidebar-top's content can't fit in the
       space sidebar-bottom leaves. Without this overflow rule it would
       visually bleed into sidebar-bottom (avatar picker rendering
       behind / on top of TODAY'S USAGE — reported in v1.0.0-nightly.3).
       overflow-y: auto turns the excess into a scrollable region with
       a thin matching scrollbar. */
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--border) transparent;
  }
  .sidebar-top::-webkit-scrollbar { width: 6px; }
  .sidebar-top::-webkit-scrollbar-track { background: transparent; }
  .sidebar-top::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 3px;
  }
  .sidebar-top::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  .account-chip {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    text-decoration: none;
    min-width: 0;
    transition: background var(--motion-fast) var(--ease-standard);
  }
  .account-chip:hover,
  .account-chip.active {
    background: var(--bg-subtle);
    color: var(--text-primary);
  }
  .account-avatar {
    flex: none;
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent-pressed);
    font-size: var(--fs-sm);
    font-weight: 700;
  }
  .account-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .account-text strong {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .account-text small {
    font-size: var(--fs-xs);
    color: var(--text-muted);
  }
  .sidebar.collapsed .account-chip {
    justify-content: center;
    padding: 6px 0;
  }

  .sidebar-bottom {
    border-top: 1px solid var(--border-subtle);
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex-shrink: 0;
  }

  /* Sidebar mascot — watercolor fox sitting in tall grass. Sits centred
     at the very bottom of the sidebar, below the usage block. The hero
     illustration is intentionally roomy (130×130) to
     feel like a real character, not a tiny icon. */
  .replay-onboarding {
    display: block;
    text-align: center;
    font-size: 11px;
    color: var(--text-secondary);
    text-decoration: none;
    padding: 6px 8px;
    margin: 4px 0 0;
    border-radius: 6px;
    transition: background 120ms ease, color 120ms ease;
  }
  .replay-onboarding:hover {
    background: var(--bg-subtle);
    color: var(--accent);
  }

  .sidebar-fox {
    margin: 6px auto -8px;
    width: 96px;
    height: 96px;
    pointer-events: none;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }
  .sidebar-fox img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 4px 8px rgba(120, 80, 30, 0.12));
    /* App-launch entrance: gentle fade + scale-up so the fox doesn't
       pop in cold. Plays once per mount; CSS handles it without JS. */
    animation: fox-arrival 700ms cubic-bezier(0.34, 1.4, 0.64, 1) both;
  }
  @keyframes fox-arrival {
    0%   { opacity: 0; transform: translateY(10px) scale(0.92); }
    60%  { opacity: 1; }
    100% { opacity: 1; transform: translateY(0) scale(1); }
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 9px;
    background: transparent;
    border: none;
    border-radius: var(--radius-md);
    font-weight: 600;
    font-size: 14px;
    color: var(--text-primary);
    cursor: pointer;
    text-align: left;
    transition: background 120ms ease;
  }

  .brand:hover {
    background: var(--bg-subtle);
  }

  .brand-mark {
    font-size: 18px;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
  }
  .brand-mark img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }


  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 5px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: var(--radius-md);
    color: var(--text-primary);
    text-decoration: none;
    font-size: 13px;
    transition: background 120ms ease;
  }

  .nav-item:hover {
    background: var(--bg-subtle);
  }

  .nav-item.active {
    background: color-mix(in srgb, var(--accent-fade) 82%, var(--bg-card));
    color: var(--accent);
    font-weight: 650;
    box-shadow: inset 3px 0 0 var(--accent), var(--shadow-xs);
  }

  .nav-icon {
    width: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .nav-item .nav-icon {
    opacity: 0.75;
  }
  .nav-item:hover .nav-icon,
  .nav-item.active .nav-icon {
    opacity: 1;
  }






  /* Hotkey reminder block */
  .hotkey-reminder {
    margin-top: 12px;
    padding: 12px 13px;
    background: var(--bg-subtle);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .hk-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 2px;
  }

  .hk-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    color: var(--text-primary);
  }

  .hk-mode {
    color: var(--text-primary);
    font-weight: 500;
  }

  kbd {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 1px 5px;
    font-family: ui-monospace, "SF Mono", Cascadia, Consolas, monospace;
    font-size: 10px;
    color: var(--text-primary);
  }

  .quick-card {
    margin-top: 8px;
    padding: 14px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-card) 78%, transparent);
    box-shadow: var(--shadow-xs);
  }

  .quick-head,
  .quick-row,
  .clean-switch,
  .companion-quick {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .quick-head {
    align-items: flex-start;
    padding-bottom: 11px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .quick-head strong {
    display: block;
    font-size: 13px;
    line-height: 1.2;
  }

  .quick-head a,
  .quick-change {
    color: var(--accent);
    font-size: 10.5px;
    font-weight: 650;
    text-decoration: none;
  }

  .quick-kicker,
  .quick-label {
    display: block;
    color: var(--text-secondary);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    line-height: 1.3;
    text-transform: uppercase;
  }

  .quick-kicker {
    margin-bottom: 3px;
    color: var(--field);
  }

  .quick-row,
  .clean-switch,
  .quick-mic,
  .companion-quick {
    padding-top: 11px;
  }

  .quick-row-copy strong,
  .companion-link strong {
    display: block;
    margin-top: 2px;
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 650;
  }

  .clean-switch {
    cursor: pointer;
  }

  .clean-switch span {
    min-width: 0;
  }

  .clean-switch strong {
    display: block;
    color: var(--text-primary);
    font-size: 12px;
  }

  .clean-switch small,
  .quick-mic small,
  .companion-link small {
    display: block;
    margin-top: 2px;
    color: var(--text-secondary);
    font-size: 9.5px;
    line-height: 1.35;
  }

  .clean-switch input {
    appearance: none;
    width: 34px;
    height: 20px;
    margin: 0;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg-subtle);
    position: relative;
    flex: 0 0 auto;
    transition: background var(--motion-fast) ease, border-color var(--motion-fast) ease;
  }

  .clean-switch input::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--bg-card);
    box-shadow: var(--shadow-xs);
    transition: transform var(--motion-base) var(--ease-standard);
  }

  .clean-switch input:checked {
    border-color: var(--accent);
    background: var(--accent);
  }

  .clean-switch input:checked::after {
    transform: translateX(14px);
  }

  .quick-mic {
    display: block;
  }

  .quick-mic select {
    width: 100%;
    height: 32px;
    margin-top: 5px;
    padding: 0 28px 0 9px;
    color: var(--text-primary);
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    font-size: 11px;
  }

  .companion-quick {
    align-items: flex-end;
  }

  .companion-link {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }

  .companion-icon {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    background: var(--bg-card);
  }

  .quick-card .vis-row {
    margin: 0;
    flex: 0 0 102px;
    padding: 2px;
    gap: 2px;
    border-radius: 9px;
    background: var(--bg-subtle);
  }

  .quick-card .vis-btn {
    border: 0;
    background: transparent;
    box-shadow: none;
  }

  .quick-card .vis-btn.active {
    background: var(--bg-card);
    box-shadow: var(--shadow-xs);
  }

  .mic-note {
    font-size: 10.5px;
    line-height: 1.4;
    color: var(--danger);
    margin-top: 4px;
  }

  /* Avatar visibility segmented control (On / Auto / Off). */
  .vis-row {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .vis-btn {
    flex: 1 1 0;
    min-width: 22px;
    padding: 4px 0;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease, box-shadow 120ms ease;
  }
  .vis-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .vis-btn.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent) inset;
  }

  /* Progress bars for usage */


  /* Collapsed usage — stacked chips, centered in the narrow sidebar. */


  .main-content {
    overflow: hidden;
    background: var(--bg-surface);
    color: var(--text-primary);
    height: 100vh;
    min-width: 0;
    transition: background 200ms ease, color 200ms ease;
  }

  /* In FLOW, not fixed. The old banner was `position: fixed; top: 10px;
     left: 50%` — it sat on top of whatever page was underneath and covered
     controls. A permission notice is not a toast: it stays until acted on, so
     it has to take up its own space rather than borrow someone else's. */
  .a11y-banner {
    margin: 0 0 14px;
    padding: 9px 12px;
    background: color-mix(in srgb, var(--warning) 9%, var(--bg-card));
    color: var(--text-primary);
    border: 1px solid color-mix(in srgb, var(--warning) 34%, transparent);
    border-radius: 10px;
    font-size: 13px;
    line-height: 1.45;
  }
  .a11y-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .a11y-status,
  .a11y-why {
    margin: 7px 0 0;
    font-size: 12.5px;
    opacity: 0.9;
  }
  .a11y-link {
    flex: 0 0 auto;
    border: none;
    background: transparent;
    padding: 0;
    color: var(--accent);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .a11y-why .a11y-link {
    margin-left: 8px;
  }
  /* macOS Accessibility nudge — floats over content (position: fixed) so it
     never disrupts page layout/scroll. Only rendered when the backend
     reports the permission is missing (i.e. macOS, not yet granted). */
  .a11y-text {
    flex: 1 1 auto;
    min-width: 220px;
  }
  .a11y-btn {
    flex: 0 0 auto;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--bg-card);
    border-radius: 7px;
    padding: 4px 10px;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    transition: opacity 120ms ease;
  }
  .a11y-btn:hover {
    opacity: 0.9;
  }
  .a11y-x {
    flex: 0 0 auto;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 13px;
    padding: 2px 4px;
    line-height: 1;
  }

  /* Narrow windows — Tauri lets the user shrink the window pretty far.
     Tighten the sidebar and drop the hero fox so things don't overlap.
     The "Replay onboarding" link also gets a smaller hit area. */
  @media (max-width: 720px) {
    .sidebar:not(.collapsed) { width: min(216px, 34vw) !important; }
    .sidebar-fox { width: 100px; height: 100px; }
  }
  @media (max-width: 560px) {
    .sidebar-fox { display: none; }
    .sidebar:not(.collapsed) { width: min(216px, 42vw) !important; }
  }

  /* Short windows — the hero fox is the biggest non-essential thing
     in sidebar-bottom. Shrink, then hide, so today's-usage + active
     models keep their space and don't fight sidebar-top for room. */
  @media (max-height: 720px) {
    .hotkey-reminder { display: none; }
    .sidebar-top { padding-top: 10px; padding-bottom: 10px; }
    .sidebar-fox { display: none; }
  }
  @media (max-height: 580px) {
    .sidebar-fox { display: none; }
  }
</style>
