<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AppContextMenu from "$lib/AppContextMenu.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { usageStore } from "$lib/usage-store.svelte";
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
    type ModelUsage,
  } from "$lib/api";
  import { account } from "$lib/account-store.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";
  import { avatarLabel } from "$lib/avatar-catalog";
  import { prettyHotkey } from "$lib/hotkey-display";
  import {
    DEEPGRAM_FREE_CREDIT_USD,
    STT_PROVIDERS,
  } from "$lib/provider-options";

  let { children } = $props();

  let collapsed = $state(false);
  let sidebarWidth = $state(272);
  let appVersion = $state<string>("");
  let flowBusy = $state(false);
  let resizingSidebar = $state(false);
  let appApiPromise: Promise<typeof import("@tauri-apps/api/app")> | null = null;

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

  // Lightweight usage meters. Deepgram shows cumulative estimated spend
  // against the current free credit; model buckets show today's audio/tokens.
  function usageFor(stage: "stt" | "llm", provider: string, model: string): ModelUsage | null {
    const rows = usageStore.usage?.model_usage ?? [];
    const exact = rows.find((r) => r.stage === stage && r.provider === provider && r.model === model);
    if (exact) return exact;
    return rows.find((r) => r.stage === stage && r.provider === provider) ?? null;
  }

  function formatAudio(seconds = 0): string {
    if (seconds < 60) return `${Math.round(seconds)}s`;
    const minutes = seconds / 60;
    if (minutes < 10) return `${minutes.toFixed(1)}m`;
    return `${Math.round(minutes)}m`;
  }

  function formatTokens(tokens = 0): string {
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(tokens >= 10_000_000 ? 0 : 1)}M`;
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(tokens >= 10_000 ? 0 : 1)}k`;
    return String(tokens);
  }

  function formatCalls(calls = 0): string {
    return calls === 1 ? "1 call" : `${calls} calls`;
  }

  let currentSttUsage = $derived(usageFor("stt", settings.s.stt_provider, settings.s.stt_model));
  let currentLlmUsage = $derived(usageFor("llm", settings.s.llm_provider, settings.s.llm_model));
  let deepgramSpend = $derived(usageStore.usage?.deepgram_estimated_usd ?? 0);
  let deepgramCredit = $derived(usageStore.usage?.deepgram_free_credit_usd ?? DEEPGRAM_FREE_CREDIT_USD);
  let deepgramPct = $derived(Math.min(100, Math.round((deepgramSpend / deepgramCredit) * 100)));
  let countSttPct = $derived(Math.min(100, Math.round(((usageStore.usage?.stt_count ?? 0) / 2000) * 100)));
  let sttAudioPct = $derived(Math.min(100, Math.round(((currentSttUsage?.audio_seconds ?? 0) / 3600) * 100)));
  let sttPct = $derived(settings.s.stt_provider === "deepgram" ? deepgramPct : sttAudioPct || countSttPct);
  let llmTokenPct = $derived(Math.min(100, Math.round(((currentLlmUsage?.total_tokens ?? 0) / 200_000) * 100)));
  let llmCallPct = $derived(Math.min(100, Math.round(((currentLlmUsage?.calls ?? usageStore.usage?.llm_count ?? 0) / 1000) * 100)));
  let llmPct = $derived(llmTokenPct || llmCallPct);
  // The 2,000-call / 3,600s / 200k-token caps are Groq free-tier numbers.
  // Deepgram has its own credit meter. For every other provider the % fill
  // is meaningless, so show the number only (empty bar track, no fake fill).
  let sttHasMeter = $derived(
    settings.s.stt_provider === "groq" || settings.s.stt_provider === "deepgram",
  );
  let llmHasMeter = $derived(settings.s.llm_provider === "groq");
  // Deepgram's line shows lifetime credit spend, not a daily "today" number.
  let sttBarKey = $derived(settings.s.stt_provider === "deepgram" ? "Credit" : "STT");
  let sttUsageLabel = $derived(
    settings.s.stt_provider === "deepgram"
      ? `$${deepgramSpend.toFixed(2)}/$${Math.round(deepgramCredit)}`
      : currentSttUsage?.audio_seconds
        ? formatAudio(currentSttUsage.audio_seconds)
        : formatCalls(currentSttUsage?.calls ?? usageStore.usage?.stt_count ?? 0),
  );
  let llmUsageLabel = $derived(
    (currentLlmUsage?.total_tokens ?? 0) > 0
      ? `${formatTokens(currentLlmUsage?.total_tokens ?? 0)} tok`
      : formatCalls(currentLlmUsage?.calls ?? usageStore.usage?.llm_count ?? 0),
  );
  let sttUsageTitle = $derived(
    `${settings.s.stt_provider} / ${settings.s.stt_model}: ${formatCalls(currentSttUsage?.calls ?? 0)}, ${formatAudio(currentSttUsage?.audio_seconds ?? 0)} today`
  );
  let llmUsageTitle = $derived(
    `${settings.s.llm_provider} / ${settings.s.llm_model}: ${formatCalls(currentLlmUsage?.calls ?? 0)}, ${formatTokens(currentLlmUsage?.total_tokens ?? 0)} tokens today`
  );

  // Daily usage rolls over at UTC midnight. Show that moment in the
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
    usageStore.subscribe();
    skinStore.subscribe();
    avatarVisibility.subscribe();

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

    // Pull the app version from Tauri (single source of truth =
    // tauri.conf.json) and show it under the brand. Helps the user track
    // which build they're testing without having to check Settings → About.
    (async () => {
      try {
        const { getVersion } = await loadAppApi();
        appVersion = await getVersion();
      } catch (e) {
        console.warn("getVersion failed", e);
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
    { href: "/history", label: "History", icon: "history" },
    { href: "/stats", label: "Stats", icon: "stats" },
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
  <div class="app-shell">
    <aside class="sidebar" class:collapsed class:resizing={resizingSidebar} style={sidebarStyle}>
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
        {#if !collapsed}
          <div class="footer-block">
            <div class="footer-title">Usage today</div>
            <div class="footer-reset" title="Model buckets reset at midnight UTC. Deepgram credit spend stays cumulative.">
              {resetLabel}
            </div>

            <div class="bar-row">
              <span class="bar-key">{sttBarKey}</span>
              <div class="bar-track">
                {#if sttHasMeter}
                  <div class="bar-fill {pctClass(sttPct)}" style="width: {sttPct}%"></div>
                {/if}
              </div>
              <span
                class="bar-val"
                title={settings.s.stt_provider === "deepgram"
                  ? `${sttUsageTitle}. Deepgram estimate: $${deepgramSpend.toFixed(2)} used at $${(usageStore.usage?.deepgram_rate_usd_per_min ?? 0.0092).toFixed(4)}/min`
                  : sttUsageTitle}
              >{sttUsageLabel}</span>
            </div>

            <div class="bar-row">
              <span class="bar-key">LLM</span>
              <div class="bar-track">
                {#if llmHasMeter}
                  <div class="bar-fill {pctClass(llmPct)}" style="width: {llmPct}%"></div>
                {/if}
              </div>
              <span class="bar-val" title={llmUsageTitle}>{llmUsageLabel}</span>
            </div>
          </div>

        {:else}
          <!-- Collapsed: stacked usage chips for STT + LLM, centered. -->
          <div class="usage-stack">
            <div class="usage-chip {sttHasMeter ? pctClass(sttPct) : 'ok'}" title={sttUsageTitle}>
              <span class="usage-chip-key">{settings.s.stt_provider === "deepgram" ? "CR" : "STT"}</span>
              <span class="usage-chip-val">{sttHasMeter ? `${sttPct}%` : "·"}</span>
            </div>
            <div class="usage-chip {llmHasMeter ? pctClass(llmPct) : 'ok'}" title={llmUsageTitle}>
              <span class="usage-chip-key">LLM</span>
              <span class="usage-chip-val">{llmHasMeter ? `${llmPct}%` : "·"}</span>
            </div>
          </div>
        {/if}
      </div>
    </aside>

    <main class="main-content">
      {#if showA11yBanner}
        <div class="a11y-banner" role="alert">
          <span class="a11y-text" title="macOS ties Accessibility to the exact app binary, so it resets after every update. If wispr-fox is already listed, remove it with – then re-add it.">
            Auto-paste needs <strong>Accessibility</strong> permission — macOS resets it after every update.
            Until you grant it, dictated text lands on the clipboard but won't paste itself.
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

  .footer-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .footer-title {
    font-size: 11px;
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
  .bar-row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) max-content;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .bar-key {
    font-weight: 500;
    color: var(--text-secondary);
  }

  .bar-track {
    height: 6px;
    background: var(--bg-subtle);
    border-radius: 3px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 300ms ease, background 200ms ease;
  }

  .bar-fill.ok { background: var(--success); }
  .bar-fill.warn { background: var(--warning); }
  .bar-fill.danger { background: var(--danger); }

  .bar-val {
    font-variant-numeric: tabular-nums;
    color: var(--text-primary);
    font-size: 11px;
    max-width: 76px;
    overflow: hidden;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    background: var(--bg-elev);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-left: 4px solid var(--warning);
    border-radius: 8px;
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
