<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import { usageStore } from "$lib/usage-store.svelte";
  import { skinStore, type Skin } from "$lib/skin-store.svelte";
  import {
    avatarVisibility,
    applyVisibilityWindow,
    type AvatarVisibility,
  } from "$lib/avatar-visibility.svelte";
  import { floaterScale, SCALE_PRESETS } from "$lib/floater-scale.svelte";
  import { settings } from "$lib/settings-store.svelte";
  import { api, onSecretsChanged, type ModelUsage, type SecretCheck } from "$lib/api";
  import { account } from "$lib/account-store.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";
  import { prettyHotkey } from "$lib/hotkey-display";
  import {
    DEEPGRAM_FREE_CREDIT_USD,
    LLM_PROVIDERS,
    STT_PROVIDERS,
    applyLlmModel,
    applyLlmProvider,
    applySttModel,
    applySttProvider,
    llmModelsFor,
    llmReady,
    sttModelsFor,
    sttReady,
  } from "$lib/provider-options";

  let { children } = $props();

  let collapsed = $state(false);
  let sidebarWidth = $state(320);
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

  type SkinOption = { id: Skin; label: string };
  const SKIN_OPTIONS: SkinOption[] = [
    { id: "fox",         label: "Fox" },
    { id: "codex-fox",   label: "Codex Fox" },
    { id: "stylized",    label: "Paperclip" },
    { id: "real-clippy", label: "Clippy" },
    { id: "cat",         label: "Desk Cat" },
    { id: "oru-gujia",   label: "Oru & Gujia" },
    { id: "spark-buddy", label: "Spark Buddy" },
    { id: "wave",        label: "Wave bar" },
    { id: "siri",        label: "Siri Orb" },
    { id: "pet-codex",       label: "Codex Pet" },
    { id: "pet-dewey",       label: "Dewey" },
    { id: "pet-fireball",    label: "Fireball" },
    { id: "pet-rocky",       label: "Rocky" },
    { id: "pet-seedy",       label: "Seedy" },
    { id: "pet-stacky",      label: "Stacky" },
    { id: "pet-bsod",        label: "BSOD" },
    { id: "pet-null-signal", label: "Null Signal" },
  ];

  // The sidebar shows a curated handful of avatars, NOT the full roster —
  // the roster kept growing (pets!) and was making the sidebar scroll. The
  // full picker lives in Settings → Appearance behind the "…" More tile.
  // If the current skin isn't featured it takes the last featured slot, so
  // the active avatar is always visible + highlighted here.
  const FEATURED_SKINS: Skin[] = ["fox", "codex-fox", "oru-gujia", "pet-codex", "wave", "siri"];
  let sidebarSkins = $derived.by<Skin[]>(() => {
    const cur = skinStore.current;
    if (FEATURED_SKINS.includes(cur)) return FEATURED_SKINS;
    return [...FEATURED_SKINS.slice(0, FEATURED_SKINS.length - 1), cur];
  });
  function skinLabel(id: Skin): string {
    return SKIN_OPTIONS.find((o) => o.id === id)?.label ?? id;
  }

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

  let secretCheck = $state<SecretCheck | null>(null);

  async function refreshSidebarSecrets() {
    try {
      secretCheck = await api.checkSecrets();
    } catch (e) {
      console.warn("sidebar secret check failed", e);
      secretCheck = null;
    }
  }

  async function pickSkin(s: Skin) {
    // Picking a skin must NOT touch visibility — the tri-state is the single
    // source of truth. The skin grid stays usable even when hidden.
    await skinStore.set(s);
  }

  async function changeSttProvider(provider: string) {
    await applySttProvider(provider);
    await usageStore.refresh();
  }

  async function changeSttModel(modelId: string) {
    await applySttModel(modelId);
    await usageStore.refresh();
  }

  async function changeLlmProvider(provider: string) {
    await applyLlmProvider(provider);
    await usageStore.refresh();
  }

  async function changeLlmModel(modelId: string) {
    await applyLlmModel(modelId);
    await usageStore.refresh();
  }

  // Avatar size presets (S/M/L). Sticky + cross-window via floaterScale.
  let scaleActive = $derived(floaterScale.activePreset());

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
    const saved = localStorage.getItem("wispr.sidebar.collapsed");
    if (saved === "1") collapsed = true;
    const savedWidth = Number(localStorage.getItem("wispr.sidebar.width"));
    if (Number.isFinite(savedWidth)) sidebarWidth = clampSidebarWidth(savedWidth);
    usageStore.subscribe();
    skinStore.subscribe();
    avatarVisibility.subscribe();
    floaterScale.subscribe();

    // Init settings, then decide whether to show the main window. The
    // window starts hidden (tauri.conf.json visible=false) so we don't
    // flash it on screen if the user wants silent startup. After settings
    // load, show it ONLY if "open_silently" is off.
    (async () => {
      await settings.init();
      await refreshSidebarSecrets();
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
    let unlistenSecrets: (() => void) | undefined;
    listen<string>("wispr:navigate", (e) => {
      goto(e.payload);
    }).then((u) => (unlisten = u));
    listen<string>("wispr:state", (e) => {
      flowBusy = e.payload !== "idle";
    }).then((u) => (unlistenFlow = u));
    // Keys can arrive from another device at any time (first sync after
    // signing in, or an edit made on web/Android). Without this the picker
    // below keeps every provider it saw as key-less greyed out and labelled
    // "- add key" for the rest of the session.
    onSecretsChanged(() => {
      refreshSidebarSecrets();
    }).then((u) => (unlistenSecrets = u));

    return () => {
      unlisten?.();
      unlistenFlow?.();
      unlistenSecrets?.();
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
    return Math.min(460, Math.max(256, Math.round(width)));
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
          <div class="section model-panel">
            <div class="section-title-bar">Models</div>

            <div class="model-row">
              <div class="model-row-head">
                <span>STT</span>
                <a href="/settings/providers" title="Manage provider keys">Keys</a>
              </div>
              <div class="model-selects">
                <select
                  aria-label="Speech-to-text service"
                  value={settings.s.stt_provider}
                  disabled={flowBusy}
                  onchange={(e) => changeSttProvider((e.currentTarget as HTMLSelectElement).value)}
                >
                  {#each STT_PROVIDERS as provider}
                    <option value={provider.id} disabled={!sttReady(secretCheck, provider.id)}>
                      {provider.label}{sttReady(secretCheck, provider.id) ? "" : " - add key"}
                    </option>
                  {/each}
                </select>
                <select
                  aria-label="Speech-to-text model"
                  value={settings.s.stt_model}
                  disabled={flowBusy}
                  onchange={(e) => changeSttModel((e.currentTarget as HTMLSelectElement).value)}
                >
                  {#each sttModelsFor(settings.s.stt_provider) as model (model.id)}
                    <option value={model.id}>{model.label}</option>
                  {/each}
                </select>
              </div>
            </div>

            <div class="model-row">
              <div class="model-row-head">
                <span>LLM</span>
                <label class="mini-toggle" title="Clean Transcribe by default">
                  <input
                    type="checkbox"
                    checked={settings.s.auto_clean_in_light}
                    disabled={flowBusy}
                    onchange={(e) => settings.set("auto_clean_in_light", (e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span>Clean</span>
                </label>
              </div>
              <div class="model-selects">
                <select
                  aria-label="LLM service"
                  value={settings.s.llm_provider}
                  disabled={flowBusy}
                  onchange={(e) => changeLlmProvider((e.currentTarget as HTMLSelectElement).value)}
                >
                  {#each LLM_PROVIDERS as provider}
                    <option value={provider.id} disabled={!llmReady(secretCheck, provider.id)}>
                      {provider.label}{llmReady(secretCheck, provider.id) ? "" : " - add key"}
                    </option>
                  {/each}
                </select>
                <select
                  aria-label="LLM model"
                  value={settings.s.llm_model}
                  disabled={flowBusy}
                  onchange={(e) => changeLlmModel((e.currentTarget as HTMLSelectElement).value)}
                >
                  {#each llmModelsFor(settings.s.llm_provider) as model (model.id)}
                    <option value={model.id}>{model.label}</option>
                  {/each}
                </select>
              </div>
            </div>
          </div>
        {/if}

        <!-- Avatar section: icon-only skin picker. Same compact grid in
             both expanded and collapsed states (just different column count). -->
        <div class="section">
          {#if !collapsed}
            <div class="section-title-bar">Avatar</div>
            <!-- Visibility tri-state — the single source of truth for whether
                 the floater is on screen. Independent of the chosen skin. -->
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
            <div class="skin-grid">
              {#each sidebarSkins as id (id)}
                <button
                  class="skin-icon-btn"
                  class:active={skinStore.current === id}
                  onclick={() => pickSkin(id)}
                  title={skinLabel(id)}
                  aria-label={skinLabel(id)}
                >
                  <SkinIcon skin={id} size={22} />
                </button>
              {/each}
              <button
                class="skin-icon-btn skin-more-btn"
                onclick={() => goto("/settings/appearance")}
                title="All avatars…"
                aria-label="All avatars"
              >…</button>
            </div>
            {#if avatarVisibility.current !== "hidden"}
              <div class="scale-row" role="group" aria-label="Avatar size">
                <span class="scale-label">Size</span>
                {#each SCALE_PRESETS as p (p.id)}
                  <button
                    class="scale-btn"
                    class:active={scaleActive === p.id}
                    onclick={() => floaterScale.set(p.value)}
                    title={`Avatar size: ${p.id === "s" ? "Small" : p.id === "m" ? "Medium" : "Large"}`}
                    aria-label={`Avatar size ${p.label}`}
                  >{p.label}</button>
                {/each}
              </div>
            {/if}
          {:else}
            <div class="vis-row-collapsed" role="group" aria-label="Avatar visibility">
              {#each VISIBILITY_OPTIONS as v (v.id)}
                <button
                  class="vis-btn"
                  class:active={avatarVisibility.current === v.id}
                  onclick={() => pickVisibility(v.id)}
                  title={v.label}
                >{v.short}</button>
              {/each}
            </div>
            <div class="skin-grid-collapsed">
              {#each sidebarSkins as id (id)}
                <button
                  class="skin-icon-btn"
                  class:active={skinStore.current === id}
                  onclick={() => pickSkin(id)}
                  title={skinLabel(id)}
                >
                  <SkinIcon skin={id} size={22} />
                </button>
              {/each}
              <button
                class="skin-icon-btn skin-more-btn"
                onclick={() => goto("/settings/appearance")}
                title="All avatars…"
                aria-label="All avatars"
              >…</button>
            </div>
            {#if avatarVisibility.current !== "hidden"}
              <div class="scale-row-collapsed" role="group" aria-label="Avatar size">
                {#each SCALE_PRESETS as p (p.id)}
                  <button
                    class="scale-btn"
                    class:active={scaleActive === p.id}
                    onclick={() => floaterScale.set(p.value)}
                    title={`Avatar size: ${p.id === "s" ? "Small" : p.id === "m" ? "Medium" : "Large"}`}
                  >{p.label}</button>
                {/each}
              </div>
            {/if}
          {/if}
        </div>

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
    width: 320px;
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
    gap: 8px;
    padding: 14px 12px;
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

  /* Avatar section — icon-only grid (same in both light + dark themes). */
  .section {
    margin-top: 16px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 12px;
  }

  .section-title-bar {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0 8px 7px;
  }

  .model-panel {
    padding-top: 12px;
  }

  .model-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 0 2px 10px;
  }

  .model-row + .model-row {
    border-top: 1px dashed var(--border-subtle);
    padding-top: 10px;
  }

  .model-row-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 2px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .model-row-head a {
    color: var(--accent);
    text-decoration: none;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0;
  }

  .model-row-head a:hover {
    color: var(--accent-hover);
    text-decoration: underline;
  }

  .model-selects {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 6px;
  }

  .model-selects select {
    min-width: 0;
    width: 100%;
    height: 30px;
    border: 1px solid var(--border-subtle);
    border-radius: 7px;
    background: var(--bg-card);
    color: var(--text-primary);
    padding: 0 8px;
    font-size: 12px;
  }

  .model-selects select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-fade);
  }

  .model-selects select:disabled,
  .mini-toggle input:disabled + span {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .mini-toggle {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    text-transform: none;
    letter-spacing: 0;
    font-size: 10px;
    font-weight: 600;
  }

  .mini-toggle input {
    width: 12px;
    height: 12px;
    margin: 0;
    accent-color: var(--accent);
  }

  .skin-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 4px;
    padding: 0 2px 6px;
  }

  .skin-grid-collapsed {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
    align-items: center;
  }

  /* Avatar size presets (S / M / L) */
  .scale-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 2px 0;
  }
  .scale-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-right: 2px;
  }
  /* Collapsed sidebar is only 56px wide. Three side-by-side S/M/L buttons
     overflowed that, so stack them vertically (matching the skin-icon column
     directly above) where they sit comfortably in the thin bar. */
  .scale-row-collapsed {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border-subtle);
  }
  .scale-btn {
    flex: 1 1 0;
    min-width: 22px;
    padding: 3px 0;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .scale-row-collapsed .scale-btn {
    flex: 0 0 auto;
    width: 36px;
    padding: 4px 0;
    font-size: 11px;
  }
  .scale-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .scale-btn.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent) inset;
  }

  /* Avatar visibility segmented control (On / Auto / Off). */
  .vis-row {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .vis-row-collapsed {
    display: flex;
    flex-direction: column;
    align-items: center;
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
    transition: all 120ms ease;
  }
  .vis-row-collapsed .vis-btn {
    flex: 0 0 auto;
    width: 36px;
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

  /* "More" tile → Settings → Appearance (the full avatar roster incl. pets). */
  .skin-more-btn {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: 1px;
    color: var(--text-secondary);
    line-height: 1;
  }
  .skin-more-btn:hover {
    color: var(--accent);
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
    .model-row { padding-bottom: 8px; }
    .model-row + .model-row { padding-top: 8px; }
    .model-selects { gap: 5px; }
    .model-selects select { height: 27px; }
    .sidebar-fox { display: none; }
  }
  @media (max-height: 580px) {
    .sidebar-fox { display: none; }
  }
</style>
