<script lang="ts">
  // 3-screen Foxy onboarding for non-technical users.
  //
  //  1. Welcome   — "Hi, I'm Foxy." Press a key, speak, get text. Three modes.
  //  2. Setup     — Smart Groq deep-link: first click opens groq.com; on
  //                 return, button copy shifts to "now take me to my keys page".
  //                 Paste field + live validation. Honest "how is this free?".
  //  3. Demo      — Pre-focused textbox + visible 5-sec timer hint. User
  //                 presses F8 right inside the onboarding window, words land
  //                 in the box. wispr:state events drive the recording UI.
  //
  // First-time users hit /onboarding automatically (auto-redirect lives in
  // the global layout). The sidebar's "Replay onboarding" link sends repeat
  // testers back here at will.

  import { onMount, onDestroy, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";

  type Screen = "welcome" | "setup" | "demo";
  let screen = $state<Screen>("welcome");

  // ── Setup state ────────────────────────────────────────────────────────
  let groqKey = $state("");
  let saving = $state(false);
  let keySaved = $state(false);
  type TestState =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; count: number }
    | { kind: "error"; msg: string };
  let testState = $state<TestState>({ kind: "idle" });

  // Smart-deeplink tracker: bump on each click. On click #2 we change the
  // button copy from "Get a Groq key" to "Take me to the keys page" so
  // returning users get a clearer second step. Same URL either way —
  // console.groq.com/keys auto-redirects through login.
  let groqClicks = $state(0);
  async function openGroqKeyPage() {
    groqClicks++;
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl("https://console.groq.com/keys");
    } catch {
      window.open("https://console.groq.com/keys", "_blank");
    }
  }

  async function verifyAndSave() {
    const k = groqKey.trim();
    if (!k) {
      testState = { kind: "error", msg: "Paste your key first." };
      return;
    }
    saving = true;
    testState = { kind: "testing" };
    try {
      const models = await api.testGroqKey(k);
      await api.saveSecret("groq_stt", k);
      await api.saveSecret("groq_llm", k);
      testState = { kind: "ok", count: models.length };
      keySaved = true;
    } catch (e) {
      testState = { kind: "error", msg: String(e) };
    } finally {
      saving = false;
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
    if (secrets.stt || secrets.llm) {
      keySaved = true;
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
  <header class="ob-head">
    <div class="brand">
      <img src="/fox/fox-logo.png" alt="" class="brand-fox" />
      <span class="brand-name">wispr-fox</span>
    </div>
    <div class="dots">
      <span class="dot {dotClass('welcome')}" title="Welcome"></span>
      <span class="dot {dotClass('setup')}" title="Setup"></span>
      <span class="dot {dotClass('demo')}" title="Try it"></span>
    </div>
  </header>

  <!-- ═════ SCREEN 1: Welcome ═════════════════════════════════════════ -->
  {#if screen === "welcome"}
    <section class="screen welcome">
      <img src="/fox/fox-hero.png" alt="" class="hero-fox" />
      <h1>Hi, I'm Foxy.</h1>
      <p class="tagline">
        Press a key, say what you mean, get it written down.
        Anywhere on your computer.
      </p>

      <div class="mode-row">
        <div class="mode-card">
          <kbd>F8</kbd>
          <h3>Raw</h3>
          <p class="example">
            <span class="said">You say:</span> "the meeting is at 4 pm tomorrow"<br />
            <span class="written">You get:</span> "the meeting is at 4 pm tomorrow"
          </p>
        </div>
        <div class="mode-card">
          <kbd>Shift+F8</kbd>
          <h3>Cleaned</h3>
          <p class="example">
            <span class="said">You say:</span> "uhh so the meeting tomorrow at 4 i think"<br />
            <span class="written">You get:</span> "The meeting tomorrow is at 4."
          </p>
        </div>
        <div class="mode-card">
          <kbd>F9</kbd>
          <h3>Drafted</h3>
          <p class="example">
            <span class="said">You say:</span> "email saurabh that i'll be late tomorrow"<br />
            <span class="written">You get:</span> "Hi Saurabh, just letting you know I'll be running late tomorrow…"
          </p>
        </div>
      </div>

      <p class="bonus">
        <strong>Bonus:</strong> Drafted mode is a hidden superpower —
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

  <!-- ═════ SCREEN 2: Setup ═══════════════════════════════════════════ -->
  {:else if screen === "setup"}
    <section class="screen setup">
      <h1>Get set up</h1>
      <p class="tagline">Two short steps and you're ready.</p>

      <details class="how-free" open>
        <summary>How is this free?</summary>
        <p>
          No secret — we piggyback on the generous personal-tier free
          credits that AI companies offer. By default we use <strong>Groq</strong>,
          which gives you <strong>2,000 transcriptions every day</strong> plus
          ~1,000 cleanups, no credit card, resets every midnight.
        </p>
        <p>
          You can change providers later or hook up paid models in Settings.
          This guide stays simple — we'll set up Groq.
        </p>
      </details>

      <div class="step-block" class:done={keySaved}>
        <div class="step-num">1</div>
        <div class="step-body">
          <h3>Get a Groq key</h3>
          {#if groqClicks === 0}
            <p class="hint">
              Opens <code>console.groq.com</code> in your browser. If you
              haven't used Groq before, sign up first (Google or GitHub
              works) — should take a minute.
            </p>
            <button class="btn primary" onclick={openGroqKeyPage}>
              Get my Groq key →
            </button>
          {:else}
            <p class="hint">
              ✓ Opened Groq in your browser.
              <strong>Signed up?</strong> Click again — this time it should
              land you straight on the API keys page.
            </p>
            <button class="btn primary" onclick={openGroqKeyPage}>
              Take me to my keys page →
            </button>
            <p class="hint subtle">
              Still stuck? Sign in, then under "API Keys" click
              "Create API Key", copy the key (starts with <code>gsk_…</code>),
              and paste it below.
            </p>
          {/if}
        </div>
      </div>

      <div class="step-block" class:done={keySaved}>
        <div class="step-num">2</div>
        <div class="step-body">
          <h3>Paste your key</h3>
          <p class="hint">
            Stored on your machine only (Windows Credential Manager) — never
            sent anywhere except Groq.
          </p>
          <div class="paste-row">
            <input
              type="password"
              placeholder="gsk_..."
              bind:value={groqKey}
              disabled={saving || keySaved}
            />
            <button
              class="btn primary"
              onclick={verifyAndSave}
              disabled={saving || keySaved || !groqKey.trim()}
            >
              {#if saving}Verifying…{:else if keySaved}Saved{:else}Verify + save{/if}
            </button>
          </div>
          {#if testState.kind === "ok"}
            <div class="status ok">✓ Key works — {testState.count} models accessible</div>
          {:else if testState.kind === "error"}
            <div class="status error">✗ {testState.msg}</div>
          {:else if testState.kind === "testing"}
            <div class="status testing">Testing key…</div>
          {/if}
        </div>
      </div>

      <p class="tip">
        Other models — Gemini, Claude, GPT, paid Groq, Sarvam for Hindi —
        all supported. Add them later in <strong>Settings → Providers</strong>.
      </p>

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
    <section class="screen demo">
      <h1>Try it now</h1>
      <p class="tagline">
        The box below is already focused. Press <kbd>F8</kbd>, say anything
        for ~5 seconds, then release.
      </p>

      <div class="demo-area">
        <textarea
          bind:this={demoBox}
          bind:value={demoText}
          class="demo-box"
          class:recording={recState === "recording"}
          class:thinking={recState === "transcribing" || recState === "cleaning" || recState === "injecting"}
          placeholder="Press F8 anywhere — your words appear here."
          rows="6"
          autofocus
        ></textarea>

        <div class="rec-ring">
          {#if recState === "recording"}
            <div class="ring listening">
              <span class="ring-label">Listening · {recElapsed.toFixed(1)}s</span>
              {#if recElapsed < 5}
                <span class="ring-hint">keep going to ~5s</span>
              {:else}
                <span class="ring-hint">good — release F8 when done</span>
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
              <span class="ring-label">Ready — press <kbd>F8</kbd> to start</span>
            </div>
          {/if}
        </div>
      </div>

      <div class="tips">
        <div class="tip-row">
          <strong>F9 instead</strong> — try it again with F9 to see Drafted
          mode. "Email John I'll be late" → a real email.
        </div>
        <div class="tip-row">
          <strong>Box not focused?</strong> Click it once, then press F8.
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
    min-height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
    display: flex;
    flex-direction: column;
    padding: 24px 32px 48px;
    max-width: 920px;
    margin: 0 auto;
  }

  .ob-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
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
    padding-top: 16px;
  }

  h1 {
    font-size: 34px;
    font-weight: 700;
    margin: 0;
    letter-spacing: -0.02em;
    color: var(--text-primary);
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

  /* ── Setup ────────────────────────────────────────────────────────── */
  .how-free {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 0;
    margin-top: 6px;
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
  .step-block.done .step-num span,
  .step-block.done .step-num {
    font-size: 14px;
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
  .step-body .hint code {
    background: var(--bg-subtle);
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 11px;
    border: 1px solid var(--border-subtle);
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

  .tip {
    background: var(--bg-subtle);
    border: 1px dashed var(--border);
    border-radius: 10px;
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
  }

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
  .ring.listening .ring-hint,
  .ring.success .ring-hint { color: inherit; opacity: 0.75; }

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

  /* Narrow window — stack the 3 mode cards vertically. */
  @media (max-width: 760px) {
    .mode-row { grid-template-columns: 1fr; }
    .hero-fox { width: 90px; }
    h1 { font-size: 28px; }
  }
</style>
