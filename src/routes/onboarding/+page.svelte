<script lang="ts">
  // 3-screen Foxy onboarding, rebuilt 2026-07-12 around three user asks:
  //
  //  1. Welcome  — the pitch ("free, unlimited, anywhere — bring your own
  //                key") + a live illustration of the actual loop: a
  //                cartoonish hotkey cap gets pressed, a pixel-pet buddy
  //                listens, transcribes, and the words type themselves into
  //                a mini textbox. The pet swaps each loop so new users see
  //                the avatar roster before they ever hit Settings.
  //  2. Setup    — "Pick your engine". Deepgram (Recommended, $200 signup
  //                credit) vs Groq (free forever). Picking one reveals the
  //                key step with an explicit fork: "I already have a key"
  //                (paste box) vs "Help me get one" (walk-through links +
  //                paste box). Saving the engine key reveals the optional
  //                cleanup-brain step (Gemini recommended; auto-covered on
  //                the Groq path since one Groq key does both jobs).
  //  3. Demo     — pre-focused textbox, press the hotkey right here, words
  //                land in the box. wispr:state events drive the UI.
  //
  // Layout contract: FLUID, full-bleed background (no fixed-width band with
  // odd gutters), and every screen is designed to fit the default 1200×800
  // window with zero scrolling. overflow-y:auto stays on as a safety net for
  // the 720×480 minimum window only.
  //
  // First-time users hit /onboarding automatically (auto-redirect lives in
  // the global layout). The sidebar's "Replay onboarding" link sends repeat
  // testers back here at will.

  import { onMount, onDestroy, tick } from "svelte";
  import { fly } from "svelte/transition";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { applySttProvider, applySttModel, applyLlmProvider } from "$lib/provider-options";
  import { prettyHotkey, isMac } from "$lib/hotkey-display";
  import SpritePet from "$lib/SpritePet.svelte";
  import { PETS } from "$lib/pets";

  // Where the OS actually stores the key — platform-aware copy in the paste step.
  const keyStoreName = isMac() ? "macOS Keychain" : "Windows Credential Manager";

  type Screen = "welcome" | "setup" | "demo";
  let screen = $state<Screen>("welcome");

  // ── Welcome-screen hero animation ────────────────────────────────────────
  // A ~9s loop acting out one dictation: wiggling key → held down (listening)
  // → transcribing → text typing into the mini box → done. The pet cycles on
  // every loop so the roster shows itself. Pure JS timeline + CSS states —
  // deliberately no 3D/canvas; it must stay light and skippable.
  type HeroPhase = "wiggle" | "listen" | "think" | "type" | "done";
  let heroPhase = $state<HeroPhase>("wiggle");
  let heroPetIdx = $state(0);
  let heroTyped = $state("");
  const HERO_LINE = "the meeting moved to 4 — see you there";

  const heroPet = $derived(PETS[heroPetIdx % PETS.length]);
  const heroAnim = $derived(
    heroPhase === "wiggle" ? "idle"
    : heroPhase === "listen" ? "waiting"
    : heroPhase === "done" ? "celebration"
    : "typing",
  );

  // Timeline driver — runs only while the welcome screen is visible.
  // prefers-reduced-motion users get a static "done" tableau instead.
  $effect(() => {
    if (screen !== "welcome") return;
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      heroPhase = "done";
      heroTyped = HERO_LINE;
      return;
    }
    let timers: ReturnType<typeof setTimeout>[] = [];
    let typeTimer: ReturnType<typeof setInterval> | null = null;
    let alive = true;
    const at = (ms: number, fn: () => void) => {
      timers.push(setTimeout(() => { if (alive) fn(); }, ms));
    };
    const runLoop = () => {
      heroPhase = "wiggle";
      heroTyped = "";
      at(1600, () => (heroPhase = "listen"));
      at(3900, () => (heroPhase = "think"));
      at(5300, () => {
        heroPhase = "type";
        let i = 0;
        typeTimer = setInterval(() => {
          i++;
          heroTyped = HERO_LINE.slice(0, i);
          if (i >= HERO_LINE.length && typeTimer) {
            clearInterval(typeTimer);
            typeTimer = null;
          }
        }, 38);
      });
      at(7200, () => (heroPhase = "done"));
      at(8900, () => {
        heroPetIdx = (heroPetIdx + 1) % PETS.length;
        runLoop();
      });
    };
    runLoop();
    return () => {
      alive = false;
      timers.forEach(clearTimeout);
      if (typeTimer) clearInterval(typeTimer);
    };
  });

  // ── Engine setup state ───────────────────────────────────────────────────
  // Two first-class engines. NO preselection — the user must actively pick
  // one before the key step reveals itself (explicit ask: the choice is
  // mandatory, the rest of the flow gates on it).
  type Engine = "deepgram" | "groq";
  let engine = $state<Engine | null>(null);

  // The explicit fork inside the key step.
  type KeyPath = "have" | "help";
  let keyPath = $state<KeyPath | null>(null);

  let primaryKey = $state("");
  let saving = $state(false);
  let keySaved = $state(false);
  type TestState =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; count: number }
    | { kind: "error"; msg: string };
  let testState = $state<TestState>({ kind: "idle" });

  // Optional cleanup "brain": Gemini is the recommended pick (generous free
  // tier, no card). On the Groq engine path this is auto-covered — one Groq
  // key powers both listening and cleanup.
  let brainKey = $state("");
  let brainSaving = $state(false);
  let brainSaved = $state(false);
  let brainTest = $state<TestState>({ kind: "idle" });
  let brainOpen = $state(false);

  // Switching engines resets the fork + paste state (but never un-saves).
  function pickEngine(e: Engine) {
    if (engine === e) return;
    engine = e;
    if (!keySaved) {
      keyPath = null;
      primaryKey = "";
      testState = { kind: "idle" };
      linkClicks = 0;
    }
  }

  const ENGINE_COPY: Record<Engine, {
    label: string;
    signupUrl: string;
    keysUrl: string;
    placeholder: string;
    keysHint: string;
  }> = {
    deepgram: {
      label: "Deepgram",
      signupUrl: "https://console.deepgram.com/signup",
      keysUrl: "https://console.deepgram.com/",
      placeholder: "paste your Deepgram key…",
      keysHint: "Sign up (Google works, no card), land in the console, then “API Keys” in the left menu → “Create a New API Key” → copy it.",
    },
    groq: {
      label: "Groq",
      signupUrl: "https://console.groq.com/keys",
      keysUrl: "https://console.groq.com/keys",
      placeholder: "gsk_...",
      keysHint: "Sign in, then under “API Keys” click “Create API Key”, copy the key (starts with gsk_…).",
    },
  };

  // Smart-deeplink tracker: click #1 opens signup; from click #2 the copy
  // shifts to "take me to my keys page" so returning users get a clearer
  // second step.
  let linkClicks = $state(0);
  async function openUrlExternal(url: string) {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } catch {
      window.open(url, "_blank");
    }
  }
  async function openEnginePage() {
    if (!engine) return;
    const url = linkClicks === 0 ? ENGINE_COPY[engine].signupUrl : ENGINE_COPY[engine].keysUrl;
    linkClicks++;
    await openUrlExternal(url);
  }

  async function verifyAndSave() {
    if (!engine) return;
    const k = primaryKey.trim();
    if (!k) {
      testState = { kind: "error", msg: "Paste your key first." };
      return;
    }
    saving = true;
    testState = { kind: "testing" };
    try {
      if (engine === "deepgram") {
        const models = await api.testDeepgramKey(k);
        await api.saveSecret("deepgram_stt", k);
        // Point the app at Deepgram Nova-3 right away — the whole pitch of
        // this path is "best ears out of the box".
        await applySttProvider("deepgram");
        await applySttModel("nova-3");
        testState = { kind: "ok", count: models.length };
      } else {
        const models = await api.testGroqKey(k);
        await api.saveSecret("groq_stt", k);
        await api.saveSecret("groq_llm", k);
        await applySttProvider("groq");
        // One Groq key covers listening AND the cleanup brain.
        brainSaved = true;
        testState = { kind: "ok", count: models.length };
      }
      keySaved = true;
    } catch (e) {
      testState = { kind: "error", msg: String(e) };
    } finally {
      saving = false;
    }
  }

  async function verifyAndSaveBrain() {
    const k = brainKey.trim();
    if (!k) {
      brainTest = { kind: "error", msg: "Paste your Gemini key first." };
      return;
    }
    brainSaving = true;
    brainTest = { kind: "testing" };
    try {
      const models = await api.testGeminiKey(k);
      await api.saveSecret("gemini_llm", k);
      await applyLlmProvider("gemini");
      brainTest = { kind: "ok", count: models.length };
      brainSaved = true;
    } catch (e) {
      brainTest = { kind: "error", msg: String(e) };
    } finally {
      brainSaving = false;
    }
  }

  // ── Demo state ─────────────────────────────────────────────────────────
  // demoBox is the textbox the user types/dictates into. We focus it on
  // mount + on screen change so the user's very first hotkey press has a
  // focused target — text injection lands directly here.
  let demoBox: HTMLTextAreaElement | undefined = $state();
  let demoText = $state("");

  // Recording state mirror driven by wispr:state events. We don't drive the
  // flow ourselves; the existing Rust hotkey/audio/STT pipeline runs as
  // normal and we just listen for state transitions to update the demo UI.
  // Payload values are the raw Rust FlowState strings.
  type RecState = "idle" | "recording" | "transcribing" | "cleaning" | "injecting";
  let recState = $state<RecState>("idle");
  let recElapsed = $state(0); // seconds while recording
  let demoCompleted = $state(false);
  let recTimer: ReturnType<typeof setInterval> | null = null;

  function startRecCounter() {
    recElapsed = 0;
    if (recTimer) clearInterval(recTimer);
    recTimer = setInterval(() => {
      recElapsed += 0.1;
    }, 100);
  }
  function stopRecCounter() {
    if (recTimer) {
      clearInterval(recTimer);
      recTimer = null;
    }
  }

  async function focusDemoBox() {
    await tick();
    demoBox?.focus();
  }

  // Watch screen change → focus demo box when we arrive at demo screen.
  $effect(() => {
    if (screen === "demo") {
      focusDemoBox();
    }
  });

  function finish() {
    goto("/history");
  }

  // "Skip" — available from every screen via a small header link. Goes to
  // History (same destination as Finish) but doesn't require keys to be
  // saved. Onboarding can be replayed any time via the sidebar's "↻ Replay
  // onboarding" link.
  function skipOnboarding() {
    goto("/history");
  }

  function replayDemo() {
    demoText = "";
    demoCompleted = false;
    recState = "idle";
    recElapsed = 0;
    focusDemoBox();
  }

  // Cleanup handles deferred from onMount — async onMount can't return a
  // teardown directly in Svelte 5 (return type is Promise<void>, not a
  // disposer). onDestroy below picks these up.
  let unlistenState: (() => void) | null = null;
  let priorTheme: string | null = null;

  onMount(async () => {
    await settings.init();
    // If user already has keys saved, just mark it — the welcome screen
    // surfaces a "Skip to the demo" shortcut for returning testers.
    const secrets = await api.checkSecrets();
    if (secrets.any_stt) {
      keySaved = true;
    }
    if (secrets.llm) {
      brainSaved = true;
    }

    unlistenState = await listen<string>("wispr:state", (e) => {
      const s = e.payload as RecState;
      recState = s;
      if (s === "recording") {
        startRecCounter();
      } else {
        stopRecCounter();
        if (s === "idle" && screen === "demo" && (demoText.trim().length > 0 || recElapsed > 0.5)) {
          demoCompleted = true;
        }
      }
    });

    // Force light theme during onboarding — first-run shouldn't be the
    // moment the user hits any dark-mode rough edges.
    priorTheme = document.body.getAttribute("data-theme");
    document.body.setAttribute("data-theme", "light");
  });

  onDestroy(() => {
    unlistenState?.();
    if (priorTheme !== null) document.body.setAttribute("data-theme", priorTheme);
    stopRecCounter();
  });

  // ── Visual helpers ─────────────────────────────────────────────────────
  function dotClass(target: Screen): string {
    const order = ["welcome", "setup", "demo"] as const;
    const cur = order.indexOf(screen);
    const t = order.indexOf(target);
    if (t < cur) return "done";
    if (t === cur) return "current";
    return "future";
  }

  const heroStatus = $derived(
    heroPhase === "wiggle" ? "hold the key…"
    : heroPhase === "listen" ? "Listening…"
    : heroPhase === "think" ? "Transcribing…"
    : heroPhase === "type" ? "Typing it for you…"
    : "Done ✓",
  );
</script>

<main class="ob">
  <!-- Ambient drifting colour fields behind everything — the "alive" layer.
       Pure CSS, pointer-events none, disabled under prefers-reduced-motion. -->
  <div class="bg-blobs" aria-hidden="true">
    <span class="blob b1"></span>
    <span class="blob b2"></span>
    <span class="blob b3"></span>
  </div>

  <div class="wrap">
    <header class="ob-head">
      <div class="brand">
        <img src="/fox/fox-logo.png" alt="" class="brand-fox" />
        <span class="brand-name">wispr-fox</span>
      </div>
      <div class="dots">
        <span class="dot {dotClass('welcome')}" title="Welcome"></span>
        <span class="dot {dotClass('setup')}" title="Get your key"></span>
        <span class="dot {dotClass('demo')}" title="Try it"></span>
      </div>
      <!-- Always-on Skip — onboarding can be replayed from the sidebar. -->
      <button class="ob-skip" onclick={skipOnboarding} title="Skip and explore the app — you can replay onboarding from the sidebar later">
        Skip →
      </button>
    </header>

    <!-- ═════ SCREEN 1: Welcome ═════════════════════════════════════════ -->
    {#if screen === "welcome"}
      <section class="screen welcome" in:fly={{ y: 22, duration: 380 }}>
        <h1 class="grad">Speech to text. Anywhere. Free.</h1>
        <p class="tagline">
          Click into any textbox on your computer, hold
          <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd>, say what you
          mean, let go — wispr-fox types it for you.
        </p>

        <!-- The loop, acted out: key press → buddy listens → transcribes →
             words land in the box. Pet swaps every loop = roster preview. -->
        <div class="hero rise" style="--d: 120ms">
          <div class="hero-key-zone">
            <div
              class="hero-key"
              class:wiggle={heroPhase === "wiggle"}
              class:held={heroPhase === "listen"}
            >
              <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd>
            </div>
            <span class="hero-key-hint">
              {heroPhase === "listen" ? "held down" : "hold to talk"}
            </span>
          </div>

          <div class="hero-pet-zone">
            <div class="hero-pet" style="--fscale: 1.35">
              <SpritePet petId={heroPet.id} anim={heroAnim} />
            </div>
            <span
              class="hero-status"
              class:listening={heroPhase === "listen"}
              class:thinking={heroPhase === "think" || heroPhase === "type"}
              class:success={heroPhase === "done"}
            >
              {#if heroPhase === "listen"}<span class="hero-wave" aria-hidden="true"><i></i><i></i><i></i><i></i></span>{/if}
              {heroStatus}
            </span>
          </div>

          <div class="hero-out-zone">
            <div class="hero-out" class:active={heroPhase === "type" || heroPhase === "done"}>
              {#if heroTyped}
                <span class="hero-out-text">{heroTyped}</span><span class="hero-caret" class:blink={heroPhase !== "done"}></span>
              {:else}
                <span class="hero-out-placeholder">your words land here — in any app</span>
              {/if}
            </div>
          </div>
        </div>

        <p class="hero-caption rise" style="--d: 220ms">
          That's <strong>{heroPet.label}</strong> — one of eight pixel buddies
          who keep you company while you dictate. There's also a watercolor
          fox, a minimal waveform, and yes, a real Clippy. Pick yours later.
        </p>

        <div class="byok rise" style="--d: 320ms">
          <strong>How is it free?</strong> wispr-fox is a
          <em>bring-your-own-key</em> app: you plug in a free API key from a
          speech provider (Deepgram, Groq Whisper…) and the app is just the
          interface. Personal free tiers are so generous that day-to-day
          dictation costs nothing — setup takes about five minutes, and the
          next screen walks you through it. Advanced users can swap engines
          and models any time in Settings.
        </div>

        <div class="cta">
          <button class="btn primary big" onclick={() => (screen = "setup")}>
            Get started →
          </button>
          {#if keySaved}
            <button class="btn ghost" onclick={() => (screen = "demo")}>
              Skip to the demo →
            </button>
          {/if}
        </div>
      </section>

    <!-- ═════ SCREEN 2: Get your key ════════════════════════════════════ -->
    {:else if screen === "setup"}
      <section class="screen setup" in:fly={{ y: 22, duration: 380 }}>
        <h1>Pick your engine</h1>
        <p class="tagline">
          Choose who does the listening — both are genuinely free to start,
          and you can switch (or add OpenAI, ElevenLabs, Gemini) later in
          <strong>Settings → Providers</strong>.
        </p>

        <div class="engine-row">
          <button
            class="engine-card rise"
            style="--d: 100ms"
            class:selected={engine === "deepgram"}
            onclick={() => pickEngine("deepgram")}
          >
            <div class="engine-head">
              <span class="engine-name">Deepgram</span>
              <span class="engine-badge rec">Recommended</span>
            </div>
            <p class="engine-pitch">
              The best ears. Nova-3 is fast and noticeably more accurate than
              older Whisper models — especially with Indian accents.
            </p>
            <p class="engine-free">
              <strong>$200 free credit</strong> on signup, no card. Heavy
              daily use ≈ <strong>$1 a week</strong> — it lasts years of
              day-to-day dictation. You'll sign up, create a key, paste it
              here — we'll walk you through it.
            </p>
          </button>
          <button
            class="engine-card rise"
            style="--d: 200ms"
            class:selected={engine === "groq"}
            onclick={() => pickEngine("groq")}
          >
            <div class="engine-head">
              <span class="engine-name">Groq</span>
              <span class="engine-badge free">Free forever</span>
            </div>
            <p class="engine-pitch">
              Unlimited wispr-fox: about 2,000 free Whisper transcriptions a
              day, resets daily, no card ever.
            </p>
            <p class="engine-free">
              One key also powers the <strong>cleanup + Draft brain</strong>
              — the all-in-one option. Sign in, create a key, paste it here.
            </p>
          </button>
        </div>

        {#if engine}
          <div class="step-block rise" style="--d: 80ms" class:done={keySaved}>
            <div class="step-num">1</div>
            <div class="step-body">
              <h3>Your {ENGINE_COPY[engine].label} key</h3>
              {#if !keySaved}
                <div class="fork-row">
                  <button
                    class="fork-btn"
                    class:selected={keyPath === "have"}
                    onclick={() => (keyPath = "have")}
                  >
                    I already have a key
                  </button>
                  <button
                    class="fork-btn"
                    class:selected={keyPath === "help"}
                    onclick={() => (keyPath = "help")}
                  >
                    Help me get one <span class="fork-sub">(free, ~3 min)</span>
                  </button>
                </div>
              {/if}

              {#if keyPath === "help" && !keySaved}
                <div class="fork-panel">
                  <p class="hint">
                    {#if linkClicks === 0}
                      Opens {ENGINE_COPY[engine].label} in your browser — sign
                      up if you haven't (Google login works), then come back
                      here.
                    {:else}
                      ✓ Opened {ENGINE_COPY[engine].label}.
                      <strong>Signed in?</strong> Click again to land on the
                      keys page, create a key, and copy it.
                    {/if}
                  </p>
                  <button class="btn primary" onclick={openEnginePage}>
                    {linkClicks === 0
                      ? `Open ${ENGINE_COPY[engine].label} — sign up or sign in →`
                      : "Take me to my keys page →"}
                  </button>
                  <p class="hint subtle">{ENGINE_COPY[engine].keysHint}</p>
                </div>
              {/if}

              {#if keyPath !== null || keySaved}
                <div class="fork-panel">
                  <p class="hint">
                    Paste it below — stored on your machine only
                    ({keyStoreName}), never sent anywhere except
                    {ENGINE_COPY[engine].label}.
                  </p>
                  <div class="paste-row">
                    <input
                      type="password"
                      placeholder={ENGINE_COPY[engine].placeholder}
                      bind:value={primaryKey}
                      disabled={saving || keySaved}
                    />
                    <button
                      class="btn primary"
                      onclick={verifyAndSave}
                      disabled={saving || keySaved || !primaryKey.trim()}
                    >
                      {#if saving}Verifying…{:else if keySaved}Saved ✓{:else}Verify + save{/if}
                    </button>
                  </div>
                  {#if testState.kind === "ok"}
                    <div class="status ok">✓ Key works — you're set for transcription</div>
                  {:else if testState.kind === "error"}
                    <div class="status error">✗ {testState.msg}</div>
                  {:else if testState.kind === "testing"}
                    <div class="status testing">Testing key…</div>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {/if}

        {#if keySaved}
          <div class="step-block rise optional" style="--d: 80ms" class:done={brainSaved}>
            <div class="step-num">2</div>
            <div class="step-body">
              <h3>
                Add a cleanup brain
                <span class="optional-tag">optional, recommended</span>
              </h3>
              {#if brainSaved}
                <p class="hint">
                  ✓ Covered — cleanup and Draft mode are powered up{engine === "groq"
                    ? " (your Groq key does both jobs)"
                    : ""}.
                </p>
              {:else}
                <p class="hint">
                  Your engine does the listening; polishing filler words and
                  Draft mode need a language model.
                  <strong>Gemini</strong> is the recommended pick — generous
                  free tier, no card. Skipping is fine: plain transcription
                  works without it, and OpenAI / Groq / Gemini can be added
                  later in Settings.
                </p>
                {#if !brainOpen}
                  <button class="btn ghost" onclick={() => (brainOpen = true)}>
                    Set up the free Gemini brain →
                  </button>
                {:else}
                  <button class="btn ghost" onclick={() => openUrlExternal("https://aistudio.google.com/apikey")}>
                    Open Google AI Studio — get an API key →
                  </button>
                  <p class="hint subtle">
                    Sign in with Google → “Create API key” → copy it → paste below.
                  </p>
                  <div class="paste-row">
                    <input
                      type="password"
                      placeholder="AIza..."
                      bind:value={brainKey}
                      disabled={brainSaving}
                    />
                    <button
                      class="btn primary"
                      onclick={verifyAndSaveBrain}
                      disabled={brainSaving || !brainKey.trim()}
                    >
                      {#if brainSaving}Verifying…{:else}Verify + save{/if}
                    </button>
                  </div>
                  {#if brainTest.kind === "ok"}
                    <div class="status ok">✓ Brain online — cleanup + Draft unlocked</div>
                  {:else if brainTest.kind === "error"}
                    <div class="status error">✗ {brainTest.msg}</div>
                  {:else if brainTest.kind === "testing"}
                    <div class="status testing">Testing key…</div>
                  {/if}
                {/if}
              {/if}
            </div>
          </div>
        {/if}

        <div class="cta">
          <button class="btn ghost" onclick={() => (screen = "welcome")}>← Back</button>
          <button
            class="btn primary big"
            onclick={() => (screen = "demo")}
            disabled={!keySaved}
          >
            Try it now →
          </button>
        </div>
      </section>

    <!-- ═════ SCREEN 3: Demo ════════════════════════════════════════════ -->
    {:else if screen === "demo"}
      <section class="screen demo" in:fly={{ y: 22, duration: 380 }}>
        <h1>Try it now</h1>
        <p class="tagline">
          The box below is already focused. Press
          <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd>, say anything
          for ~5 seconds, then release.
        </p>

        <div class="demo-area">
          <textarea
            bind:this={demoBox}
            bind:value={demoText}
            class="demo-box"
            class:recording={recState === "recording"}
            class:thinking={recState === "transcribing" || recState === "cleaning" || recState === "injecting"}
            placeholder="Press {prettyHotkey(settings.s.light_hotkey)} anywhere — your words appear here."
            rows="5"
          ></textarea>

          <div class="rec-ring">
            {#if recState === "recording"}
              <div class="ring listening">
                <span class="ring-label">Listening · {recElapsed.toFixed(1)}s</span>
                {#if recElapsed < 5}
                  <span class="ring-hint">keep going to ~5s</span>
                {:else}
                  <span class="ring-hint">good — release {prettyHotkey(settings.s.light_hotkey)} when done</span>
                {/if}
              </div>
            {:else if recState === "transcribing"}
              <div class="ring thinking">
                <span class="ring-label">Transcribing…</span>
              </div>
            {:else if recState === "cleaning" || recState === "injecting"}
              <div class="ring thinking">
                <span class="ring-label">Writing it down…</span>
              </div>
            {:else if demoCompleted}
              <div class="ring success">
                <span class="ring-label">✓ Nice — that's it. Try another?</span>
                <button class="btn ghost small" onclick={replayDemo}>Clear</button>
              </div>
            {:else}
              <div class="ring idle">
                <span class="ring-label">Ready — press <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd> to start</span>
              </div>
            {/if}
          </div>
        </div>

        <div class="tips">
          <div class="tip-row">
            <strong>{prettyHotkey(settings.s.drafting_hotkey)} instead</strong>
            — Draft mode writes the whole thing from the gist. "Email John
            I'll be late" → a real email.
          </div>
          <div class="tip-row">
            <strong>Need to bail mid-recording?</strong>
            Press <kbd>Esc</kbd> — stops cleanly without sending anything.
          </div>
          <div class="tip-row">
            <strong>Change hotkeys or providers</strong> any time in Settings.
          </div>
        </div>

        <div class="cta">
          <button class="btn ghost" onclick={() => (screen = "setup")}>← Back</button>
          <button class="btn primary big" onclick={finish}>Finish →</button>
        </div>
      </section>
    {/if}
  </div>
</main>

<style>
  :global(body) {
    background: var(--bg-surface);
  }

  /* Full-bleed page: the background paints edge-to-edge at ANY window size
     (the old layout capped the whole page at 920px and centered it, which
     read as odd gutters on wide windows). Content centers inside .wrap. */
  .ob {
    height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
    overflow: hidden;
    position: relative;
  }

  .wrap {
    height: 100%;
    max-width: 1040px;
    margin: 0 auto;
    padding: 20px 40px 0;
    display: flex;
    flex-direction: column;
    position: relative;
    z-index: 1;
  }

  /* ── Ambient drifting colour fields ──────────────────────────────────
     Three big blurred blobs slowly wandering behind the content. Warm
     accent tones at low opacity so they read as light, not decoration. */
  .bg-blobs {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    z-index: 0;
  }
  .blob {
    position: absolute;
    border-radius: 50%;
    filter: blur(70px);
    opacity: 0.16;
  }
  .blob.b1 {
    width: 380px;
    height: 380px;
    background: #ec7c34;
    top: -120px;
    left: -100px;
    animation: drift-a 38s ease-in-out infinite alternate;
  }
  .blob.b2 {
    width: 300px;
    height: 300px;
    background: #f0b429;
    bottom: -80px;
    right: -60px;
    animation: drift-b 46s ease-in-out infinite alternate;
  }
  .blob.b3 {
    width: 220px;
    height: 220px;
    background: #e8956b;
    top: 40%;
    left: 55%;
    opacity: 0.10;
    animation: drift-a 52s ease-in-out infinite alternate-reverse;
  }
  @keyframes drift-a {
    0%   { transform: translate(0, 0) scale(1); }
    100% { transform: translate(120px, 60px) scale(1.15); }
  }
  @keyframes drift-b {
    0%   { transform: translate(0, 0) scale(1); }
    100% { transform: translate(-100px, -70px) scale(1.1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .blob { animation: none; }
    .rise { animation: none; opacity: 1; }
    .hero-key.wiggle { animation: none; }
  }

  /* Staggered entrance for cards/blocks — set --d per element. */
  .rise {
    animation: rise-in 480ms cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: var(--d, 0ms);
  }
  @keyframes rise-in {
    0%   { opacity: 0; transform: translateY(14px); }
    100% { opacity: 1; transform: translateY(0); }
  }

  .ob-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
    flex-shrink: 0;
    gap: 12px;
  }

  .ob-skip {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-secondary);
    border-radius: 999px;
    padding: 4px 12px;
    font-size: 12px;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .ob-skip:hover {
    color: var(--text-primary);
    border-color: var(--text-secondary);
    background: var(--bg-subtle);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .brand-fox {
    width: 28px;
    height: 28px;
    object-fit: contain;
  }

  .brand-name {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-secondary);
    letter-spacing: 0.02em;
  }

  .dots {
    display: flex;
    gap: 8px;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 9999px;
    background: var(--border);
    transition: background 200ms ease;
  }
  .dot.done { background: var(--accent-soft); }
  .dot.current { background: var(--accent); box-shadow: 0 0 0 4px var(--accent-fade); }

  .screen {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 10px 0 32px;
    /* Safety net only: every screen is sized to fit the default 1200×800
       window with NO scrolling; the scrollbar exists for the 720×480
       minimum window so the CTA always stays reachable. */
    overflow-y: auto;
    min-height: 0;
    scrollbar-width: thin;
    scrollbar-color: var(--border) transparent;
  }
  .screen::-webkit-scrollbar { width: 8px; }
  .screen::-webkit-scrollbar-track { background: transparent; }
  .screen::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 4px;
  }
  .screen::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  h1 {
    font-size: 32px;
    font-weight: 700;
    margin: 0;
    letter-spacing: -0.02em;
    color: var(--text-primary);
  }
  /* Warm gradient headline — the one "award-site" flourish on each screen. */
  h1.grad {
    background: linear-gradient(100deg, #d9542b, var(--accent) 45%, #f0a429);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  .tagline {
    font-size: 15px;
    color: var(--text-secondary);
    margin: 0;
    max-width: 640px;
    line-height: 1.55;
  }

  /* ── Welcome hero ─────────────────────────────────────────────────── */
  .welcome h1, .welcome .tagline {
    align-self: center;
    text-align: center;
  }

  .hero {
    display: grid;
    grid-template-columns: 150px 190px 1fr;
    align-items: center;
    gap: 22px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 22px 28px;
    margin-top: 6px;
    min-height: 208px;
  }

  .hero-key-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }
  .hero-key {
    display: grid;
    place-items: center;
    transition: transform 160ms ease;
  }
  .hero-key kbd {
    font-size: 22px;
    padding: 14px 22px;
    border-radius: 12px;
    border: 2px solid var(--border);
    border-bottom-width: 6px;
    background: var(--bg-surface);
    box-shadow: 0 3px 0 rgba(43, 34, 24, 0.06);
    transition: transform 140ms ease, border-bottom-width 140ms ease, background 140ms ease;
  }
  .hero-key.wiggle { animation: key-wiggle 900ms ease-in-out infinite; }
  @keyframes key-wiggle {
    0%, 100% { transform: rotate(0deg); }
    25%      { transform: rotate(-4deg) scale(1.03); }
    75%      { transform: rotate(4deg) scale(1.03); }
  }
  .hero-key.held kbd {
    transform: translateY(4px);
    border-bottom-width: 2px;
    background: var(--accent-fade);
    border-color: var(--accent);
  }
  .hero-key-hint {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .hero-pet-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .hero-pet {
    display: grid;
    place-items: center;
  }
  .hero-status {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    font-weight: 500;
    padding: 5px 14px;
    border-radius: 9999px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    white-space: nowrap;
    transition: all 200ms ease;
  }
  .hero-status.listening {
    background: var(--danger-fade);
    color: var(--danger);
    border-color: var(--danger-fade);
  }
  .hero-status.thinking {
    background: var(--info-fade);
    color: var(--info);
    border-color: var(--info-fade);
  }
  .hero-status.success {
    background: var(--success-fade);
    color: var(--success);
    border-color: var(--success-fade);
  }

  /* Tiny live "waveform" inside the listening pill. */
  .hero-wave {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
  }
  .hero-wave i {
    width: 3px;
    background: currentColor;
    border-radius: 2px;
    animation: wavebar 700ms ease-in-out infinite;
  }
  .hero-wave i:nth-child(1) { animation-delay: 0ms; }
  .hero-wave i:nth-child(2) { animation-delay: 140ms; }
  .hero-wave i:nth-child(3) { animation-delay: 280ms; }
  .hero-wave i:nth-child(4) { animation-delay: 420ms; }
  @keyframes wavebar {
    0%, 100% { height: 4px; }
    50%      { height: 12px; }
  }

  .hero-out-zone { min-width: 0; }
  .hero-out {
    background: var(--bg-surface);
    border: 2px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
    min-height: 76px;
    font-size: 14px;
    line-height: 1.5;
    display: flex;
    align-items: center;
    transition: border-color 200ms ease, box-shadow 200ms ease;
  }
  .hero-out.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-fade);
  }
  .hero-out-text { color: var(--text-primary); }
  .hero-out-placeholder { color: var(--text-muted); font-style: italic; }
  .hero-caret {
    display: inline-block;
    width: 2px;
    height: 1.1em;
    background: var(--accent);
    margin-left: 2px;
    vertical-align: text-bottom;
  }
  .hero-caret.blink { animation: caret-blink 900ms step-end infinite; }
  @keyframes caret-blink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }

  .hero-caption {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0;
    align-self: center;
    text-align: center;
    max-width: 640px;
  }
  .hero-caption strong { color: var(--accent); }

  .byok {
    background: var(--accent-fade);
    border: 1px solid var(--accent-soft);
    border-radius: 12px;
    padding: 13px 16px;
    font-size: 13px;
    color: var(--text-primary);
    line-height: 1.55;
    margin: 0;
  }

  /* ── Engine setup ─────────────────────────────────────────────────── */
  .engine-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    margin-top: 4px;
  }
  .engine-card {
    text-align: left;
    background: var(--bg-card);
    border: 2px solid var(--border);
    border-radius: 16px;
    padding: 14px 18px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 7px;
    font-family: inherit;
    color: var(--text-primary);
    transition: transform 160ms ease, border-color 160ms ease, box-shadow 160ms ease;
  }
  .engine-card:hover {
    transform: translateY(-2px);
    border-color: var(--accent-soft);
  }
  .engine-card.selected {
    border-color: var(--accent);
    background: var(--accent-fade);
    box-shadow: 0 6px 20px rgba(184, 84, 18, 0.12);
  }
  .engine-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .engine-name {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .engine-badge {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px 9px;
    border-radius: 999px;
  }
  .engine-badge.rec {
    background: var(--accent);
    color: #fff;
  }
  .engine-badge.free {
    background: var(--success-fade);
    color: var(--success);
    border: 1px solid var(--success-fade);
  }
  .engine-pitch {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
  }
  .engine-free {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
  }
  .engine-free strong { color: var(--text-primary); }

  .optional-tag {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    padding: 1px 8px;
    margin-left: 6px;
    vertical-align: 2px;
  }

  .step-block {
    display: grid;
    grid-template-columns: 36px 1fr;
    gap: 14px;
    padding: 14px 18px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 14px;
    transition: opacity 200ms ease, border-color 200ms ease;
  }
  .step-block.done {
    opacity: 0.72;
    border-color: var(--success-fade);
  }

  .step-num {
    width: 30px;
    height: 30px;
    border-radius: 9999px;
    background: var(--accent-fade);
    color: var(--accent);
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
  }
  .step-block.done .step-num {
    background: var(--success-fade);
    color: var(--success);
    font-size: 0; /* hide the number, show only the check */
  }
  .step-block.done .step-num::before {
    content: "✓";
    font-size: 14px;
  }

  .step-body h3 {
    font-size: 15px;
    margin: 0 0 8px;
    color: var(--text-primary);
  }

  .step-body .hint {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0 0 10px;
  }
  .step-body .hint.subtle {
    font-size: 12px;
    margin-top: 8px;
  }

  /* The mandatory fork: already-have vs help-me-get. */
  .fork-row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 4px;
  }
  .fork-btn {
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    background: var(--bg-surface);
    border: 2px solid var(--border);
    border-radius: 10px;
    padding: 9px 16px;
    cursor: pointer;
    transition: border-color 140ms ease, background 140ms ease, transform 80ms ease;
  }
  .fork-btn:hover { border-color: var(--accent-soft); }
  .fork-btn.selected {
    border-color: var(--accent);
    background: var(--accent-fade);
  }
  .fork-btn .fork-sub {
    color: var(--text-muted);
    font-weight: 400;
    font-size: 12px;
  }

  .fork-panel {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px dashed var(--border-subtle);
  }

  .paste-row {
    display: flex;
    gap: 8px;
    align-items: stretch;
    margin-bottom: 6px;
  }
  .paste-row input {
    flex: 1;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 12px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    background: var(--bg-surface);
    color: var(--text-primary);
    transition: border-color 120ms ease, box-shadow 120ms ease;
  }
  .paste-row input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-fade);
  }

  .status {
    font-size: 12px;
    margin-top: 4px;
    padding: 6px 0;
  }
  .status.ok      { color: var(--success); font-weight: 500; }
  .status.error   { color: var(--danger); }
  .status.testing { color: var(--text-secondary); }

  /* ── Demo ─────────────────────────────────────────────────────────── */
  .demo-area {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 6px;
  }

  .demo-box {
    width: 100%;
    min-height: 120px;
    padding: 16px 18px;
    font-size: 15px;
    line-height: 1.55;
    border: 2px solid var(--border);
    border-radius: 14px;
    background: var(--bg-card);
    color: var(--text-primary);
    resize: vertical;
    font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
    transition: border-color 180ms ease, box-shadow 180ms ease;
  }
  .demo-box:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-fade);
  }
  .demo-box.recording {
    border-color: var(--danger);
    box-shadow: 0 0 0 4px var(--danger-fade);
    animation: pulse 1.4s ease-in-out infinite;
  }
  .demo-box.thinking {
    border-color: var(--info);
    box-shadow: 0 0 0 4px var(--info-fade);
  }
  @keyframes pulse {
    0%, 100% { box-shadow: 0 0 0 4px var(--danger-fade); }
    50%      { box-shadow: 0 0 0 6px var(--danger-fade); }
  }

  .rec-ring {
    align-self: center;
  }
  .ring {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 10px 18px;
    border-radius: 9999px;
    font-size: 13px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
  }
  .ring.listening {
    background: var(--danger-fade);
    color: var(--danger);
    border-color: var(--danger-fade);
  }
  .ring.thinking {
    background: var(--info-fade);
    color: var(--info);
    border-color: var(--info-fade);
  }
  .ring.success {
    background: var(--success-fade);
    color: var(--success);
    border-color: var(--success-fade);
  }
  .ring-label { font-weight: 500; }
  .ring-hint { color: var(--text-secondary); font-size: 12px; }
  .ring.listening .ring-hint { color: inherit; opacity: 0.75; }

  .tips {
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--bg-subtle);
    border-radius: 12px;
    padding: 14px 16px;
    margin-top: 6px;
  }
  .tip-row {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .tip-row strong { color: var(--text-primary); }

  /* ── Shared CTA / buttons ─────────────────────────────────────────── */
  .cta {
    display: flex;
    gap: 10px;
    align-items: center;
    justify-content: center;
    margin-top: auto;
    padding-top: 16px;
    flex-wrap: wrap;
  }
  .setup .cta, .demo .cta { justify-content: flex-start; }

  .btn {
    border: none;
    cursor: pointer;
    border-radius: 9px;
    font-size: 13px;
    font-weight: 500;
    padding: 9px 18px;
    transition: background 120ms ease, transform 80ms ease;
  }
  .btn.primary {
    background: var(--accent);
    color: #fff;
  }
  .btn.primary:hover:not(:disabled) { background: var(--accent-hover); }
  .btn.primary:active:not(:disabled) { background: var(--accent-pressed); transform: translateY(1px); }
  .btn.primary:disabled { opacity: 0.45; cursor: not-allowed; }
  .btn.primary.big {
    padding: 12px 26px;
    font-size: 14px;
  }
  .btn.ghost {
    background: transparent;
    color: var(--text-secondary);
  }
  .btn.ghost:hover { color: var(--text-primary); background: var(--bg-subtle); }
  .btn.ghost.small { padding: 4px 10px; font-size: 11px; }

  kbd {
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 5px;
    padding: 2px 6px;
    font-size: 11px;
    color: var(--text-primary);
  }

  /* Narrow window — stack the hero and cards vertically. */
  @media (max-width: 860px) {
    .hero {
      grid-template-columns: 1fr;
      justify-items: center;
      gap: 14px;
      padding: 18px;
    }
    .hero-out-zone { width: 100%; }
    .engine-row { grid-template-columns: 1fr; }
    h1 { font-size: 27px; }
    .wrap { padding: 16px 20px 0; }
  }
</style>
