<script lang="ts">
  // 3-screen Foxy onboarding for non-technical users.
  //
  //  1. Welcome   — "Hi, I'm Foxy." Press a key, speak, get text. Three modes,
  //                 animated entrance, ambient gradient blobs.
  //  2. Engine    — provider-neutral setup. Two selectable engine cards:
  //                 Deepgram (Recommended — $200 signup credit, Nova-3, best
  //                 with Indian accents) and Groq (free forever). Smart
  //                 deep-links: first click opens signup, second click lands
  //                 on the keys page. Paste field + live verification. When
  //                 Deepgram is chosen, an optional third step adds the free
  //                 Groq "brain" for cleanup/Draft mode.
  //  3. Demo      — Pre-focused textbox + visible 5-sec timer hint. User
  //                 presses F8 right inside the onboarding window, words land
  //                 in the box. wispr:state events drive the recording UI.
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
  import { applySttProvider, applySttModel } from "$lib/provider-options";
  import { prettyHotkey, isMac } from "$lib/hotkey-display";

  // Where the OS actually stores the key — platform-aware copy in Setup step 2.
  const keyStoreName = isMac() ? "macOS Keychain" : "Windows Credential Manager";

  type Screen = "welcome" | "setup" | "demo";
  let screen = $state<Screen>("welcome");

  // ── Engine setup state ───────────────────────────────────────────────────
  // Two first-class engines. Deepgram is the recommended default (better
  // accuracy + speed than Whisper, especially for Indian English; $200 free
  // signup credit ≈ a year of heavy use). Groq stays as the free-forever
  // path and doubles as the cleanup/draft "brain" either way.
  type Engine = "deepgram" | "groq";
  let engine = $state<Engine>("deepgram");

  let primaryKey = $state("");
  let saving = $state(false);
  let keySaved = $state(false);
  type TestState =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; count: number }
    | { kind: "error"; msg: string };
  let testState = $state<TestState>({ kind: "idle" });

  // Optional "brain" step (only shown on the Deepgram path): a free Groq key
  // so cleanup + Draft mode work. Groq-path users get this for free (one key
  // does both jobs).
  let brainKey = $state("");
  let brainSaving = $state(false);
  let brainSaved = $state(false);
  let brainTest = $state<TestState>({ kind: "idle" });

  // Switching engines resets the paste/verify state (but never un-saves).
  function pickEngine(e: Engine) {
    if (engine === e) return;
    engine = e;
    if (!keySaved) {
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

  // Smart-deeplink tracker: bump on each click. Click #1 opens signup;
  // from click #2 the copy shifts to "take me to my keys page" so returning
  // users get a clearer second step.
  let linkClicks = $state(0);
  async function openEnginePage() {
    const url = linkClicks === 0 ? ENGINE_COPY[engine].signupUrl : ENGINE_COPY[engine].keysUrl;
    linkClicks++;
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
    } catch {
      window.open(url, "_blank");
    }
  }

  let brainClicks = $state(0);
  async function openBrainPage() {
    brainClicks++;
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl("https://console.groq.com/keys");
    } catch {
      window.open("https://console.groq.com/keys", "_blank");
    }
  }

  async function verifyAndSave() {
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
      brainTest = { kind: "error", msg: "Paste your Groq key first." };
      return;
    }
    brainSaving = true;
    brainTest = { kind: "testing" };
    try {
      const models = await api.testGroqKey(k);
      await api.saveSecret("groq_llm", k);
      // Also usable as an STT fallback if Deepgram credit ever runs dry.
      await api.saveSecret("groq_stt", k);
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
  // mount + on screen change so the user's very first F8 press has a focused
  // target — text injection lands directly here.
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
  // Friendly app name from Rust; empty until first F8 press fires
  // wispr:active_app. Used only in the tip line; the demo box itself is
  // always the target since it's the focused element.
  let activeApp = $state("");

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

  // "Skip — I'll set this up later" — available from every screen via a small
  // header link. Goes to History (same destination as Finish) but doesn't
  // require keys to be saved. Reported as a major friction point on a Mac
  // where the STT provider was blocked at the corporate egress and the user
  // couldn't verify a key but also couldn't get past the setup screen.
  // Onboarding can be replayed any time via the sidebar's "↻ Replay
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
  let unlistenActiveApp: (() => void) | null = null;
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

    unlistenActiveApp = await listen<string>("wispr:active_app", (e) => {
      activeApp = e.payload ?? "";
    });

    // Force light theme during onboarding — first-run shouldn't be the
    // moment the user hits any dark-mode rough edges.
    priorTheme = document.body.getAttribute("data-theme");
    document.body.setAttribute("data-theme", "light");
  });

  onDestroy(() => {
    unlistenState?.();
    unlistenActiveApp?.();
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
</script>

<main class="ob">
  <!-- Ambient drifting colour fields behind everything — the "alive" layer.
       Pure CSS, pointer-events none, disabled under prefers-reduced-motion. -->
  <div class="bg-blobs" aria-hidden="true">
    <span class="blob b1"></span>
    <span class="blob b2"></span>
    <span class="blob b3"></span>
  </div>

  <header class="ob-head">
    <div class="brand">
      <img src="/fox/fox-logo.png" alt="" class="brand-fox" />
      <span class="brand-name">wispr-fox</span>
    </div>
    <div class="dots">
      <span class="dot {dotClass('welcome')}" title="Welcome"></span>
      <span class="dot {dotClass('setup')}" title="Pick your engine"></span>
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
      <img src="/fox/fox-hero.png" alt="" class="hero-fox" />
      <h1 class="grad">Hi, I'm Foxy.</h1>
      <p class="tagline">
        Press a key, say what you mean, get it written down.
        Anywhere on your computer.
      </p>
      <p class="type-demo">
        <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd>
        <span class="type-text">"chalo — let's ship this today"</span>
      </p>

      <div class="mode-row">
        <div class="mode-card rise" style="--d: 120ms">
          <kbd>{prettyHotkey(settings.s.light_hotkey)}</kbd>
          <h3>Transcribe</h3>
          <p class="example">
            <span class="said">You say:</span> "the meeting is at 4 pm tomorrow"<br />
            <span class="written">You get:</span> "the meeting is at 4 pm tomorrow"
          </p>
        </div>
        <div class="mode-card rise" style="--d: 220ms">
          <kbd>{prettyHotkey(settings.s.force_clean_hotkey)}</kbd>
          <h3>Transcribe + clean</h3>
          <p class="example">
            <span class="said">You say:</span> "uhh so the meeting tomorrow at 4 i think"<br />
            <span class="written">You get:</span> "The meeting tomorrow is at 4."
          </p>
        </div>
        <div class="mode-card rise" style="--d: 320ms">
          <kbd>{prettyHotkey(settings.s.drafting_hotkey)}</kbd>
          <h3>Draft</h3>
          <p class="example">
            <span class="said">You say:</span> "email saurabh that i'll be late tomorrow"<br />
            <span class="written">You get:</span> "Hi Saurabh, just letting you know I'll be running late tomorrow…"
          </p>
        </div>
      </div>

      <p class="bonus rise" style="--d: 420ms">
        <strong>Bonus:</strong> Draft mode is a hidden superpower —
        say the gist of what you want and it writes the whole thing for you.
        Email, Slack, doc, anything.
      </p>

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

  <!-- ═════ SCREEN 2: Pick your engine ════════════════════════════════ -->
  {:else if screen === "setup"}
    <section class="screen setup" in:fly={{ y: 22, duration: 380 }}>
      <h1>Pick your engine</h1>
      <p class="tagline">
        One key and you're dictating. Both options are genuinely free to
        start — pick one, we'll walk you to the key.
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
            The best ears. Nova-3 is faster and noticeably more accurate than
            the older Whisper models — especially with Indian accents.
          </p>
          <p class="engine-free">
            <strong>$200 free credit</strong> on signup, no card. Heavy daily
            use burns about <strong>$1 a week</strong> — it lasts a year+.
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
            About 2,000 free transcriptions a day (Whisper), resets daily,
            no card.
          </p>
          <p class="engine-free">
            One key also powers the <strong>cleanup + Draft brain</strong> —
            the all-in-one option.
          </p>
        </button>
      </div>

      <details class="how-free">
        <summary>How is this free?</summary>
        <p>
          No secret — AI companies like Deepgram, Groq, and Google court
          developers with generous personal free tiers and signup credits.
          A dictation app sips tokens, so those allowances go a very long way.
        </p>
        <p>
          You can switch providers or add others (OpenAI, ElevenLabs, Gemini)
          any time in <strong>Settings → Providers</strong>.
        </p>
      </details>

      <div class="step-block rise" style="--d: 280ms" class:done={keySaved}>
        <div class="step-num">1</div>
        <div class="step-body">
          <h3>Get your {ENGINE_COPY[engine].label} key</h3>
          {#if linkClicks === 0}
            <p class="hint">
              Opens {ENGINE_COPY[engine].label} in your browser — sign up if
              you haven't (Google login works, takes a minute), and keep this
              window open.
            </p>
            <button class="btn primary" onclick={openEnginePage}>
              Get my {ENGINE_COPY[engine].label} key →
            </button>
          {:else}
            <p class="hint">
              ✓ Opened {ENGINE_COPY[engine].label} in your browser.
              <strong>Signed up?</strong> Click again to land on the keys page.
            </p>
            <button class="btn primary" onclick={openEnginePage}>
              Take me to my keys page →
            </button>
            <p class="hint subtle">
              Still stuck? {ENGINE_COPY[engine].keysHint}
            </p>
          {/if}
        </div>
      </div>

      <div class="step-block rise" style="--d: 360ms" class:done={keySaved}>
        <div class="step-num">2</div>
        <div class="step-body">
          <h3>Paste your key</h3>
          <p class="hint">
            Stored on your machine only ({keyStoreName}) — never sent anywhere
            except {ENGINE_COPY[engine].label}.
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
              {#if saving}Verifying…{:else if keySaved}Saved{:else}Verify + save{/if}
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
      </div>

      {#if engine === "deepgram"}
        <div class="step-block rise optional" style="--d: 440ms" class:done={brainSaved}>
          <div class="step-num">3</div>
          <div class="step-body">
            <h3>Add the free brain <span class="optional-tag">optional, recommended</span></h3>
            {#if brainSaved}
              <p class="hint">✓ Done — cleanup and Draft mode are powered up.</p>
            {:else}
              <p class="hint">
                Deepgram does the listening; cleanup + Draft mode need a
                language model. Groq's free tier covers that (no card) —
                grab a key the same way and paste it here. Skipping is fine:
                plain transcription works without it.
              </p>
              <button class="btn ghost" onclick={openBrainPage}>
                {brainClicks === 0 ? "Get a free Groq key →" : "Take me to the Groq keys page →"}
              </button>
              <div class="paste-row">
                <input
                  type="password"
                  placeholder="gsk_..."
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
          rows="6"
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
          — try it again with {prettyHotkey(settings.s.drafting_hotkey)} to
          see Draft mode. "Email John I'll be late" → a real email.
        </div>
        <div class="tip-row">
          <strong>Need to bail mid-recording?</strong>
          Press <kbd>Esc</kbd> — stops cleanly without sending anything.
        </div>
        <div class="tip-row">
          <strong>Box not focused?</strong> Click it once, then press
          {prettyHotkey(settings.s.light_hotkey)}.
        </div>
        <div class="tip-row">
          <strong>Change hotkeys</strong> any time in
          Settings → Dictation.
        </div>
      </div>

      <div class="cta">
        <button class="btn ghost" onclick={() => (screen = "setup")}>← Back</button>
        <button class="btn primary big" onclick={finish}>Finish →</button>
      </div>
    </section>
  {/if}
</main>

<style>
  :global(body) {
    background: var(--bg-surface);
  }

  .ob {
    /* Was min-height: 100vh — meant the layout could grow taller than the
       viewport and the CTA (Finish / Skip) would slide off-screen with no
       way to reach it. Now: pin to the viewport height, keep the header
       sticky, and let the SCREEN section own its own scroll. The screen
       padding-bottom gives the CTA breathing room above the window edge. */
    height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
    display: flex;
    flex-direction: column;
    padding: 24px 32px 0;
    max-width: 920px;
    margin: 0 auto;
    overflow: hidden;
    position: relative;
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
    .type-text { animation: none; width: auto; border-right: none; }
  }

  /* Everything above the blobs. */
  .ob-head, .screen { position: relative; z-index: 1; }

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
    margin-bottom: 8px;
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
    gap: 18px;
    padding: 16px 0 48px;
    /* Critical: own the overflow so on a short window the user can scroll
       to the CTA buttons. Previously content pushed Finish off-screen
       with no scroll bar (reported on a 1366×768 Mac at the Setup screen). */
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
    font-size: 34px;
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
    max-width: 580px;
    line-height: 1.55;
  }

  /* ── Welcome ──────────────────────────────────────────────────────── */
  .welcome { align-items: flex-start; }
  .hero-fox {
    width: 110px;
    height: auto;
    align-self: center;
    margin-bottom: -4px;
    animation: foxArrival 600ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  @keyframes foxArrival {
    0%   { opacity: 0; transform: translateY(8px) scale(0.92); }
    100% { opacity: 1; transform: translateY(0) scale(1); }
  }

  .welcome h1, .welcome .tagline {
    align-self: center;
    text-align: center;
  }

  /* Looping-free typewriter line: types once, keeps a soft blinking caret. */
  .type-demo {
    align-self: center;
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 2px 0 0;
    font-size: 13px;
    color: var(--text-secondary);
  }
  .type-text {
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    white-space: nowrap;
    overflow: hidden;
    border-right: 2px solid var(--accent);
    width: 31ch;
    animation:
      typing 2.2s steps(31, end) 700ms both,
      caret 900ms step-end infinite;
  }
  @keyframes typing { from { width: 0; } to { width: 31ch; } }
  @keyframes caret { 0%, 100% { border-color: var(--accent); } 50% { border-color: transparent; } }

  .mode-row {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
    margin-top: 12px;
    width: 100%;
  }

  .mode-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: transform 160ms ease, box-shadow 160ms ease, border-color 160ms ease;
  }
  .mode-card:hover {
    transform: translateY(-3px);
    border-color: var(--accent-soft);
    box-shadow: 0 8px 22px rgba(184, 84, 18, 0.10);
  }

  .mode-card kbd {
    align-self: flex-start;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 6px;
    padding: 2px 8px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 11px;
    color: var(--text-primary);
  }

  .mode-card h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    color: var(--text-primary);
  }

  .mode-card .example {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0;
  }

  .mode-card .said,
  .mode-card .written {
    font-weight: 600;
    color: var(--text-muted);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    display: inline-block;
    min-width: 56px;
  }

  .bonus {
    background: var(--accent-fade);
    border: 1px solid var(--accent-soft);
    border-radius: 12px;
    padding: 14px 16px;
    font-size: 13px;
    color: var(--text-primary);
    line-height: 1.55;
    margin: 4px 0 0;
  }

  /* ── Engine setup ─────────────────────────────────────────────────── */
  .engine-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    margin-top: 6px;
  }
  .engine-card {
    text-align: left;
    background: var(--bg-card);
    border: 2px solid var(--border);
    border-radius: 16px;
    padding: 16px 18px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 8px;
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
  .step-block.optional .paste-row { margin-top: 10px; }

  .how-free {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 0;
    margin-top: 0;
  }
  .how-free > summary {
    cursor: pointer;
    padding: 12px 16px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    list-style: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .how-free > summary::before {
    content: "▸";
    color: var(--text-secondary);
    font-size: 10px;
    transition: transform 150ms ease;
    display: inline-block;
  }
  .how-free[open] > summary::before {
    transform: rotate(90deg);
  }
  .how-free > summary::-webkit-details-marker { display: none; }
  .how-free > p {
    margin: 0;
    padding: 0 16px 12px 30px;
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
  }
  .how-free > p:last-child {
    padding-bottom: 16px;
  }

  .step-block {
    display: grid;
    grid-template-columns: 36px 1fr;
    gap: 14px;
    padding: 16px 18px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 14px;
    transition: opacity 200ms ease, border-color 200ms ease;
  }
  .step-block.done {
    opacity: 0.65;
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
  }
  .step-block.done .step-num::before {
    content: "✓";
  }

  .step-body h3 {
    font-size: 15px;
    margin: 0 0 6px;
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
    min-height: 130px;
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
    margin-top: 8px;
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
    margin-top: 20px;
    flex-wrap: wrap;
  }

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

  /* Narrow window — stack the cards vertically. */
  @media (max-width: 760px) {
    .mode-row { grid-template-columns: 1fr; }
    .engine-row { grid-template-columns: 1fr; }
    .hero-fox { width: 90px; }
    h1 { font-size: 28px; }
    .type-demo { display: none; }
  }
</style>
