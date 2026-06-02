<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import { usageStore } from "$lib/usage-store.svelte";
  import { skinStore, setClippyWindowVisible, type Skin } from "$lib/skin-store.svelte";
  import { settings } from "$lib/settings-store.svelte";
  import { api } from "$lib/api";
  import SkinIcon from "$lib/SkinIcon.svelte";
  import { prettyHotkey } from "$lib/hotkey-display";

  let { children } = $props();

  let collapsed = $state(false);
  let appVersion = $state<string>("");

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
    } catch (e) {
      console.warn("accessibility check failed", e);
      accessibilityOk = true; // fail open — never nag if the check itself errors
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
  });

  type SkinOption = { id: Skin; label: string };
  const SKIN_OPTIONS: SkinOption[] = [
    { id: "off",         label: "Off" },
    { id: "fox",         label: "Fox" },
    { id: "stylized",    label: "Paperclip" },
    { id: "real-clippy", label: "Clippy" },
    { id: "cat",         label: "Desk Cat" },
    { id: "cat-lab",     label: "Cat (lab)" },
  ];

  async function pickSkin(s: Skin) {
    await skinStore.set(s);
    await setClippyWindowVisible(s !== "off");
  }

  // Percentage of daily-limit used (Groq free tier: 2000 STT, 1000 LLM per UTC day).
  let sttPct = $derived(Math.min(100, Math.round(((usageStore.usage?.stt_count ?? 0) / 2000) * 100)));
  let llmPct = $derived(Math.min(100, Math.round(((usageStore.usage?.llm_count ?? 0) / 1000) * 100)));

  // Groq's daily limits reset at UTC midnight. Show that moment in the
  // user's local timezone so they don't have to do mental UTC math
  // (especially relevant for IST users who are +5:30 from UTC).
  function nextUtcMidnightLocal(): string {
    const now = new Date();
    const next = new Date(Date.UTC(
      now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1, 0, 0, 0
    ));
    const hours = Math.floor((next.getTime() - now.getTime()) / 3_600_000);
    const mins = Math.floor((next.getTime() - now.getTime()) / 60_000) % 60;
    const timeStr = next.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    if (hours >= 1) return `resets at ${timeStr} (${hours}h ${mins}m)`;
    return `resets at ${timeStr} (${mins}m)`;
  }
  // Reactive, re-evaluates on each render. Cheap enough.
  let resetLabel = $state(nextUtcMidnightLocal());
  // Refresh every minute so the countdown stays accurate.
  onMount(() => {
    const t = setInterval(() => { resetLabel = nextUtcMidnightLocal(); }, 60_000);
    return () => clearInterval(t);
  });
  function pctClass(p: number): string {
    if (p < 50) return "ok";
    if (p < 85) return "warn";
    return "danger";
  }

  // Persist sidebar collapsed state across launches.
  onMount(() => {
    const saved = localStorage.getItem("wispr.sidebar.collapsed");
    if (saved === "1") collapsed = true;
    usageStore.subscribe();
    skinStore.subscribe();

    // Init settings, then decide whether to show the main window. The
    // window starts hidden (tauri.conf.json visible=false) so we don't
    // flash it on screen if the user wants silent startup. After settings
    // load, show it ONLY if "open_silently" is off.
    (async () => {
      await settings.init();
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

    // Pull the app version from Tauri (single source of truth =
    // tauri.conf.json) and show it under the brand. Helps the user track
    // which build they're testing without having to check Settings → About.
    (async () => {
      try {
        const { getVersion } = await import("@tauri-apps/api/app");
        appVersion = await getVersion();
      } catch (e) {
        console.warn("getVersion failed", e);
      }
    })();

    // Tray menu can request navigation via wispr:navigate event.
    let unlisten: (() => void) | undefined;
    listen<string>("wispr:navigate", (e) => {
      goto(e.payload);
    }).then((u) => (unlisten = u));

    return () => unlisten?.();
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

  // Hide chrome on /onboarding (full-bleed) and /clippy (floating window).
  let hideChrome = $derived(
    page.url?.pathname?.startsWith("/onboarding") ||
    page.url?.pathname?.startsWith("/clippy") ||
    false,
  );

  type NavItem = { href: string; label: string; icon: string };
  const navItems: NavItem[] = [
    { href: "/history", label: "History", icon: "🕓" },
    { href: "/settings", label: "Settings", icon: "⚙" },
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

  function shortModel(name: string | undefined): string {
    if (!name) return "—";
    // "llama-3.3-70b-versatile" → "llama 3.3 70b"
    // "whisper-large-v3-turbo" → "whisper turbo"
    if (name.startsWith("whisper-large-v3-turbo")) return "whisper turbo";
    if (name.startsWith("whisper-large-v3")) return "whisper v3";
    if (name.startsWith("distil-whisper")) return "distil-whisper";
    if (name.startsWith("llama-3.3-70b")) return "llama 70b";
    if (name.startsWith("llama-3.1-8b")) return "llama 8b";
    return name.replace(/-/g, " ");
  }
</script>

{#if hideChrome}
  {@render children?.()}
{:else}
  <div class="app-shell">
    <aside class="sidebar" class:collapsed>
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
            <span class="brand-text">
              wispr-fox
              {#if appVersion}<span class="brand-version">v{appVersion}</span>{/if}
            </span>
          {/if}
        </button>

        <nav class="nav">
          {#each navItems as item (item.href)}
            <a href={item.href} class="nav-item" class:active={isActive(item.href)}>
              <span class="nav-icon">{item.icon}</span>
              {#if !collapsed}<span class="nav-label">{item.label}</span>{/if}
            </a>
          {/each}
        </nav>

        {#if !collapsed}
          <div class="hotkey-reminder">
            <div class="hk-title">HOLD TO DICTATE</div>
            <div class="hk-row">
              <span class="hk-mode">Light</span>
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

        <!-- Floater section: icon-only skin picker. Same compact grid in
             both expanded and collapsed states (just different column count). -->
        <div class="section">
          {#if !collapsed}
            <div class="section-title-bar">FLOATER</div>
            <div class="skin-grid">
              {#each SKIN_OPTIONS as opt (opt.id)}
                <button
                  class="skin-icon-btn"
                  class:active={skinStore.current === opt.id}
                  onclick={() => pickSkin(opt.id)}
                  title={opt.label}
                  aria-label={opt.label}
                >
                  <SkinIcon skin={opt.id} size={26} />
                </button>
              {/each}
            </div>
          {:else}
            <div class="skin-grid-collapsed">
              {#each SKIN_OPTIONS as opt (opt.id)}
                <button
                  class="skin-icon-btn"
                  class:active={skinStore.current === opt.id}
                  onclick={() => pickSkin(opt.id)}
                  title={opt.label}
                >
                  <SkinIcon skin={opt.id} size={22} />
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <div class="sidebar-bottom">
        {#if !collapsed}
          <div class="footer-block">
            <div class="footer-title">Today's usage</div>
            <div class="footer-reset" title="Groq free-tier limits reset at midnight UTC">{resetLabel}</div>

            <div class="bar-row">
              <span class="bar-key">STT</span>
              <div class="bar-track">
                <div class="bar-fill {pctClass(sttPct)}" style="width: {sttPct}%"></div>
              </div>
              <span class="bar-val">{usageStore.usage?.stt_count ?? 0}/2000</span>
            </div>

            <div class="bar-row">
              <span class="bar-key">LLM</span>
              <div class="bar-track">
                <div class="bar-fill {pctClass(llmPct)}" style="width: {llmPct}%"></div>
              </div>
              <span class="bar-val">{usageStore.usage?.llm_count ?? 0}/1000</span>
            </div>
          </div>

          <div class="footer-block">
            <div class="footer-title">Active models</div>
            <div class="footer-stat" title={usageStore.models?.stt}>
              <span class="footer-key">STT</span>
              <span class="footer-val">{shortModel(usageStore.models?.stt)}</span>
            </div>
            <div class="footer-stat" title={usageStore.models?.llm_advanced}>
              <span class="footer-key">LLM</span>
              <span class="footer-val">{shortModel(usageStore.models?.llm_advanced)}</span>
            </div>
          </div>
        {:else}
          <!-- Collapsed: stacked usage chips for STT + LLM, centered. -->
          <div class="usage-stack">
            <div class="usage-chip {pctClass(sttPct)}" title="STT (Whisper): {usageStore.usage?.stt_count ?? 0} of 2000 today">
              <span class="usage-chip-key">STT</span>
              <span class="usage-chip-val">{sttPct}%</span>
            </div>
            <div class="usage-chip {pctClass(llmPct)}" title="LLM (cleanup): {usageStore.usage?.llm_count ?? 0} of 1000 today">
              <span class="usage-chip-key">LLM</span>
              <span class="usage-chip-val">{llmPct}%</span>
            </div>
          </div>
        {/if}

        <!-- Replay onboarding — small dev-mode helper so testers can re-walk
             the 3-screen flow without nuking their keys. TODO: hide behind
             a settings.dev_mode flag once the flow is locked. -->
        {#if !collapsed}
          <a class="replay-onboarding" href="/onboarding" data-sveltekit-preload-data="off">
            ↻ Replay onboarding
          </a>
        {/if}

        <!-- Sidebar fox mascot. Placeholder inline SVG (a stylised fox face)
             until the user-provided pastoral fox illustration lands. Hidden
             when the sidebar is collapsed since it'd just be visual noise
             at 56px wide. The eventual replacement asset is a small fox
             sitting in grass — see RELEASE_NOTES_v0.4.0 asset wishlist. -->
        {#if !collapsed}
          <div class="sidebar-fox" aria-hidden="true">
            <!-- Pastoral watercolor fox in tall grass — the calm, ambient
                 mascot at the bottom of the sidebar from the design
                 playbook. Replaces v0.4.0's inline-SVG placeholder. -->
            <img src="/fox/fox-hero.png" alt="" />
          </div>
        {/if}
      </div>
    </aside>

    <main class="main-content">
      {#if showA11yBanner}
        <div class="a11y-banner" role="alert">
          <span class="a11y-text">
            Auto-paste needs <strong>Accessibility</strong> permission. Until you grant it,
            dictated text is copied to your clipboard but won't paste itself.
          </span>
          <button class="a11y-btn" onclick={grantAccessibility}>Open Settings</button>
          <button class="a11y-btn ghost" onclick={() => checkAccessibility()}>Re-check</button>
          <button class="a11y-x" onclick={() => (a11yDismissed = true)} aria-label="Dismiss">✕</button>
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
    display: flex;
    flex-direction: column;
    width: 200px;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
    transition: width 180ms cubic-bezier(0.32, 0.72, 0, 1),
                background 200ms ease,
                border-color 200ms ease;
    overflow: hidden;
    color: var(--text-primary);
  }

  .sidebar.collapsed {
    width: 56px;
  }

  .sidebar-top {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 8px;
    min-height: 0;
    /* When window is short, sidebar-top's content can't fit in the
       space sidebar-bottom leaves. Without this overflow rule it would
       visually bleed into sidebar-bottom (FLOATER picker rendering
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

  .sidebar-bottom {
    border-top: 1px solid var(--border-subtle);
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex-shrink: 0;
  }

  /* Sidebar mascot — watercolor fox sitting in tall grass. Sits centred
     at the very bottom of the sidebar, below usage + active-models
     blocks. The hero illustration is intentionally roomy (130×130) to
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
    margin: 8px auto -4px;
    width: 130px;
    height: 130px;
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
    padding: 8px 10px;
    background: transparent;
    border: none;
    border-radius: 8px;
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
    width: 22px;
    height: 22px;
  }
  .brand-mark img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .brand-version {
    margin-left: 6px;
    font-size: 10px;
    color: var(--text-secondary);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 8px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 7px;
    color: var(--text-primary);
    text-decoration: none;
    font-size: 13px;
    transition: background 120ms ease;
  }

  .nav-item:hover {
    background: var(--bg-subtle);
  }

  .nav-item.active {
    background: var(--accent-fade);
    color: var(--accent);
    font-weight: 500;
  }

  .nav-icon {
    width: 18px;
    text-align: center;
    font-size: 13px;
  }

  .footer-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .footer-title {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .footer-reset {
    font-size: 10px;
    color: var(--text-secondary);
    margin-top: -2px;
    margin-bottom: 4px;
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }

  .footer-stat {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-size: 11px;
  }

  .footer-key {
    color: var(--text-secondary);
  }

  .footer-val {
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
    font-feature-settings: "tnum";
  }

  /* Hotkey reminder block */
  .hotkey-reminder {
    margin-top: 12px;
    padding: 10px 10px;
    background: var(--bg-subtle);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .hk-title {
    font-size: 10px;
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

  /* Floater section — icon-only grid (same in both light + dark themes). */
  .section {
    margin-top: 14px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 12px;
  }

  .section-title-bar {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0 10px 8px;
  }

  .skin-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 4px;
    padding: 0 2px 8px;
  }

  .skin-grid-collapsed {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
    align-items: center;
  }

  .skin-icon-btn {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    border-radius: 8px;
    width: 100%;
    aspect-ratio: 1;
    color: var(--text-primary);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    transition: all 120ms ease;
  }

  .skin-grid-collapsed .skin-icon-btn {
    width: 36px;
    aspect-ratio: 1;
  }

  .skin-icon-btn:hover {
    border-color: var(--accent);
    transform: translateY(-1px);
  }

  .skin-icon-btn.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent) inset;
  }

  /* Progress bars for usage */
  .bar-row {
    display: grid;
    grid-template-columns: 26px 1fr auto;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: #6e6e73;
  }

  .bar-key {
    font-weight: 500;
    color: #6e6e73;
  }

  .bar-track {
    height: 6px;
    background: rgba(0, 0, 0, 0.06);
    border-radius: 3px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 300ms ease, background 200ms ease;
  }

  .bar-fill.ok { background: #34c759; }
  .bar-fill.warn { background: #ff9f0a; }
  .bar-fill.danger { background: #ff453a; }

  .bar-val {
    font-variant-numeric: tabular-nums;
    color: #1d1d1f;
    font-size: 10px;
  }

  /* Collapsed usage — stacked chips, centered in the narrow sidebar. */
  .usage-stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
    width: 100%;
  }

  .usage-chip {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .usage-chip.ok { background: var(--success-fade); color: var(--success); }
  .usage-chip.warn { background: var(--warning-fade); color: var(--warning); }
  .usage-chip.danger { background: var(--danger-fade); color: var(--danger); }

  .usage-chip-key {
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.04em;
    margin-bottom: 2px;
    opacity: 0.85;
  }

  .usage-chip-val {
    font-size: 11px;
    font-weight: 600;
  }

  .main-content {
    overflow: hidden;
    background: var(--bg-surface);
    color: var(--text-primary);
    height: 100vh;
    min-width: 0;
    transition: background 200ms ease, color 200ms ease;
  }

  /* macOS Accessibility nudge — floats over content (position: fixed) so it
     never disrupts page layout/scroll. Only rendered when the backend
     reports the permission is missing (i.e. macOS, not yet granted). */
  .a11y-banner {
    position: fixed;
    top: 10px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1000;
    max-width: min(680px, calc(100vw - 80px));
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    background: var(--warning-fade, #fff4e0);
    color: var(--text-primary);
    border: 1px solid var(--warning, #ff9f0a);
    border-radius: 10px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.14);
    font-size: 12px;
    line-height: 1.35;
  }
  .a11y-text {
    flex: 1 1 auto;
  }
  .a11y-btn {
    flex: 0 0 auto;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
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
  .a11y-btn.ghost {
    background: transparent;
    color: var(--accent);
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
    .sidebar { width: 172px; }
    .sidebar-fox { width: 100px; height: 100px; }
  }
  @media (max-width: 560px) {
    .sidebar-fox { display: none; }
    .sidebar { width: 156px; }
  }

  /* Short windows — the hero fox is the biggest non-essential thing
     in sidebar-bottom. Shrink, then hide, so today's-usage + active
     models keep their space and don't fight sidebar-top for room. */
  @media (max-height: 720px) {
    .sidebar-fox { width: 96px; height: 96px; }
  }
  @media (max-height: 580px) {
    .sidebar-fox { display: none; }
  }
</style>
