<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import ClippyDialog from "$lib/ClippyDialog.svelte";

  type Step =
    | "welcome"
    | "provider"
    | "get-key"
    | "paste-key"
    | "test"
    | "hotkeys"
    | "done";

  let step = $state<Step>("welcome");
  let provider = $state<"groq" | "gemini" | "skip">("groq");
  let groqKey = $state("");
  let geminiKey = $state("");
  let saving = $state(false);
  let testState = $state<
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; count: number }
    | { kind: "error"; msg: string }
  >({ kind: "idle" });

  onMount(async () => {
    await settings.init();
    const secrets = await api.checkSecrets();
    // If they've already configured something, skip onboarding entirely.
    if (secrets.stt || secrets.gemini) {
      goto("/history");
    }
  });

  function next(s: Step) {
    step = s;
  }

  async function saveAndTest() {
    saving = true;
    testState = { kind: "testing" };
    try {
      if (provider === "groq") {
        const k = groqKey.trim();
        if (!k) {
          testState = { kind: "error", msg: "Paste your key first." };
          saving = false;
          return;
        }
        const models = await api.testGroqKey(k);
        // Save both STT and LLM as same key (common case).
        await api.saveSecret("groq_stt", k);
        await api.saveSecret("groq_llm", k);
        testState = { kind: "ok", count: models.length };
      } else if (provider === "gemini") {
        const k = geminiKey.trim();
        if (!k) {
          testState = { kind: "error", msg: "Paste your key first." };
          saving = false;
          return;
        }
        const models = await api.testGeminiKey(k);
        await api.saveSecret("gemini_llm", k);
        // Default Gemini as the LLM provider for all modes when chosen here.
        await settings.set("light_provider", "gemini");
        await settings.set("advanced_provider", "gemini");
        await settings.set("drafting_provider", "gemini");
        await settings.set("clippy_light_model", "gemini-2.5-flash");
        await settings.set("clippy_advanced_model", "gemini-2.5-flash");
        await settings.set("clippy_drafting_model", "gemini-2.5-flash");
        testState = { kind: "ok", count: models.length };
      }
      setTimeout(() => next("hotkeys"), 600);
    } catch (e) {
      testState = { kind: "error", msg: String(e) };
    } finally {
      saving = false;
    }
  }

  function display(combo: string): string {
    return combo
      .replace(/CommandOrControl/g, "Ctrl")
      .replace(/Super/g, "Win")
      .replace(/\+/g, " + ");
  }

  function finish() {
    goto("/history");
  }

  // Step → Clippy message map
  let clippyMessage = $derived.by(() => {
    switch (step) {
      case "welcome":
        return "Hi there! I'm Clippy. Let's get wispr-fox set up — takes about a minute. You'll dictate anywhere with a hotkey and I'll paste it for you.";
      case "provider":
        return "First pick an AI service. Groq is recommended — fastest, generous free tier, no credit card. Gemini is the second-best free option. You can change this anytime.";
      case "get-key":
        return provider === "groq"
          ? "Click the link below to open Groq's console. Sign in with Google or GitHub, click 'Create API Key', copy it, come back here."
          : "Click the link below to open Google AI Studio. Sign in with your Google account, create an API key, copy it, come back here.";
      case "paste-key":
        return "Paste the key here and I'll verify it works.";
      case "test":
        return "Verifying your key…";
      case "hotkeys":
        return "Three hotkeys you'll use everywhere. Hold any of them anywhere on Windows, speak, release.";
      case "done":
        return "All set! Try it now — open any text field and hold F8 while you talk. You can change anything in Settings.";
    }
  });
</script>

<main class="onboarding" class:home={step === "welcome" || step === "done"}>
  <div class="content">
    <!-- Clippy + dialog header — always present, message swaps per step -->
    <header class="head">
      <ClippyDialog message={clippyMessage} size={86} pose="default" direction="right" />
      <div class="step-pill">Step {
        step === "welcome" ? 1 :
        step === "provider" ? 2 :
        step === "get-key" ? 3 :
        step === "paste-key" || step === "test" ? 4 :
        step === "hotkeys" ? 5 : 6
      } of 6</div>
    </header>

    <!-- Step body -->
    <section class="body">
      {#if step === "welcome"}
        <h1>Welcome to wispr-fox</h1>
        <p class="lede">
          A keyboard-driven dictation app. Hold a global hotkey, speak, release — your words appear in whatever
          text field has focus. Three modes for different writing tasks. Free to use with most providers.
        </p>
        <div class="feature-grid">
          <div class="feature">
            <span class="feature-key">F8</span>
            <div class="feature-text">
              <strong>Light</strong>
              <p>Quick dictation — raw transcript</p>
            </div>
          </div>
          <div class="feature">
            <span class="feature-key">F9</span>
            <div class="feature-text">
              <strong>Advanced</strong>
              <p>Treats your dictation as instructions</p>
            </div>
          </div>
          <div class="feature">
            <span class="feature-key">F10</span>
            <div class="feature-text">
              <strong>Drafting</strong>
              <p>Drafts polished output from a brief</p>
            </div>
          </div>
        </div>
        <div class="actions">
          <button class="primary" onclick={() => next("provider")}>Let's go →</button>
        </div>

      {:else if step === "provider"}
        <h2>Pick your AI service</h2>
        <div class="provider-grid">
          <button
            class="provider-card"
            class:selected={provider === "groq"}
            onclick={() => (provider = "groq")}
          >
            <div class="provider-card-head">
              <span class="provider-card-title">Groq</span>
              <span class="badge badge-recommended">Recommended</span>
            </div>
            <ul class="provider-bullets">
              <li><strong>Free tier:</strong> 2,000 STT + 1,000–14,400 LLM req/day</li>
              <li><strong>Speed:</strong> Fastest inference</li>
              <li><strong>Card:</strong> Not required</li>
              <li><strong>Models:</strong> Whisper + Llama</li>
            </ul>
          </button>

          <button
            class="provider-card"
            class:selected={provider === "gemini"}
            onclick={() => (provider = "gemini")}
          >
            <div class="provider-card-head">
              <span class="provider-card-title">Google Gemini</span>
              <span class="badge badge-alt">Best free LLM</span>
            </div>
            <ul class="provider-bullets">
              <li><strong>Free tier:</strong> 1,500 LLM req/day on Flash</li>
              <li><strong>Speed:</strong> Fast, competitive quality</li>
              <li><strong>Card:</strong> Not required</li>
              <li><strong>Note:</strong> LLM only — still uses Groq for STT</li>
            </ul>
          </button>

          <button
            class="provider-card subtle"
            class:selected={provider === "skip"}
            onclick={() => (provider = "skip")}
          >
            <div class="provider-card-head">
              <span class="provider-card-title">Set up later</span>
            </div>
            <p class="muted">
              Skip for now. You can add API keys later in Settings → Provider & Keys.
              The app won't work until you do.
            </p>
          </button>
        </div>
        <div class="actions">
          <button class="ghost" onclick={() => next("welcome")}>← Back</button>
          <button
            class="primary"
            onclick={() => provider === "skip" ? next("hotkeys") : next("get-key")}
          >
            Continue →
          </button>
        </div>

      {:else if step === "get-key"}
        <h2>Get your {provider === "groq" ? "Groq" : "Gemini"} API key</h2>
        <div class="key-steps">
          <ol>
            {#if provider === "groq"}
              <li>Click the button below — opens <code>console.groq.com</code> in your browser</li>
              <li>Sign in with Google, GitHub, or email</li>
              <li>Click <strong>"Create API Key"</strong>, give it a name (e.g. "wispr-fox")</li>
              <li>Copy the key (starts with <code>gsk_…</code>)</li>
              <li>Come back here</li>
            {:else}
              <li>Click the button below — opens <code>aistudio.google.com</code></li>
              <li>Sign in with your Google account</li>
              <li>Click <strong>"Create API key"</strong></li>
              <li>Copy the key (starts with <code>AIza…</code>)</li>
              <li>Come back here</li>
            {/if}
          </ol>
        </div>
        <div class="actions">
          <a
            class="primary external"
            href={provider === "groq"
              ? "https://console.groq.com/keys"
              : "https://aistudio.google.com/app/apikey"}
            target="_blank"
            rel="noopener"
          >
            Open {provider === "groq" ? "Groq" : "Gemini"} console ↗
          </a>
          <button class="ghost" onclick={() => next("provider")}>← Back</button>
          <button class="primary" onclick={() => next("paste-key")}>I have my key →</button>
        </div>

      {:else if step === "paste-key" || step === "test"}
        <h2>Paste your key</h2>
        <p class="lede">
          Your key is stored locally — Windows Credential Manager (primary) plus a JSON file fallback.
          Never sent anywhere except {provider === "groq" ? "Groq" : "Google"}'s servers.
        </p>
        <div class="key-input-block">
          <label>
            <span>API key</span>
            {#if provider === "groq"}
              <input
                type="password"
                placeholder="gsk_..."
                bind:value={groqKey}
                disabled={saving}
              />
            {:else}
              <input
                type="password"
                placeholder="AIza..."
                bind:value={geminiKey}
                disabled={saving}
              />
            {/if}
          </label>
          {#if testState.kind === "ok"}
            <div class="status status-ok">
              ✓ Key works — {testState.count} models accessible. Saving + continuing…
            </div>
          {:else if testState.kind === "error"}
            <div class="status status-error">✗ {testState.msg}</div>
          {:else if testState.kind === "testing"}
            <div class="status status-testing">Testing…</div>
          {/if}
        </div>
        <div class="actions">
          <button class="ghost" onclick={() => next("get-key")}>← Back</button>
          <button class="primary" onclick={saveAndTest} disabled={saving}>
            {saving ? "Verifying…" : "Verify + save →"}
          </button>
        </div>

      {:else if step === "hotkeys"}
        <h2>Your hotkeys</h2>
        <p class="lede">
          These work anywhere on Windows. Hold to record (push-to-talk), release when done.
          Win+F8/F9/F10 are sticky variants — press once to start, press again to stop. All
          configurable in Settings later.
        </p>
        <div class="hotkey-list">
          <div class="hotkey-card">
            <kbd>{display(settings.s.light_hotkey)}</kbd>
            <div class="hotkey-text">
              <strong>Light</strong>
              <p>Raw Whisper transcript. No LLM. Use for quick dictation.</p>
            </div>
          </div>
          <div class="hotkey-card">
            <kbd>{display(settings.s.advanced_hotkey)}</kbd>
            <div class="hotkey-text">
              <strong>Advanced</strong>
              <p>Treats dictation as instructions. "Make this an email", "shorter", "in bullets".</p>
            </div>
          </div>
          <div class="hotkey-card">
            <kbd>{display(settings.s.drafting_hotkey)}</kbd>
            <div class="hotkey-text">
              <strong>Drafting</strong>
              <p>Drafts polished output from a brief. Best for emails, Slack messages, docs.</p>
            </div>
          </div>
        </div>
        <div class="actions">
          <button class="ghost" onclick={() => next("paste-key")}>← Back</button>
          <button class="primary" onclick={() => next("done")}>Looks good →</button>
        </div>

      {:else if step === "done"}
        <h1>You're ready</h1>
        <p class="lede">
          Try it now: open any text field anywhere on your computer, hold <kbd>F8</kbd>,
          say something, and release. The transcript appears where you were typing.
          Recordings show up in History.
        </p>
        <div class="actions">
          <button class="primary big" onclick={finish}>Open the app →</button>
        </div>
        <p class="muted small" style="margin-top: 16px;">
          You can change providers, hotkeys, themes, and floater character in Settings.
        </p>
      {/if}
    </section>
  </div>
</main>

<style>
  .onboarding {
    min-height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }

  .content {
    width: 100%;
    max-width: 760px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 32px 40px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.06);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 24px;
    padding-bottom: 20px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .step-pill {
    background: var(--bg-subtle);
    color: var(--text-secondary);
    padding: 4px 12px;
    border-radius: 9999px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .body h1 {
    font-size: 28px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0 0 12px;
    letter-spacing: -0.02em;
  }

  .body h2 {
    font-size: 22px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 14px;
  }

  .lede {
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1.6;
    margin: 0 0 22px;
  }

  .muted {
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.5;
    margin: 0;
  }
  .muted.small {
    font-size: 12px;
  }

  /* Feature grid on welcome step */
  .feature-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin: 20px 0 24px;
  }

  .feature {
    background: var(--bg-subtle);
    border-radius: 12px;
    padding: 14px;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .feature-key {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 6px;
    padding: 6px 10px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 13px;
    color: var(--text-primary);
    flex-shrink: 0;
  }

  .feature-text strong {
    display: block;
    font-size: 13px;
    color: var(--text-primary);
    margin-bottom: 2px;
  }

  .feature-text p {
    margin: 0;
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  /* Provider cards */
  .provider-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 24px;
  }

  .provider-card {
    background: var(--bg-card);
    border: 1.5px solid var(--border);
    border-radius: 12px;
    padding: 16px 18px;
    cursor: pointer;
    text-align: left;
    transition: all 150ms ease;
    color: var(--text-primary);
  }

  .provider-card:hover {
    border-color: var(--accent);
  }

  .provider-card.selected {
    border-color: var(--accent);
    background: var(--accent-fade);
    box-shadow: 0 0 0 2px var(--accent-fade);
  }

  .provider-card.subtle {
    background: var(--bg-subtle);
    border-style: dashed;
  }

  .provider-card-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }

  .provider-card-title {
    font-size: 15px;
    font-weight: 600;
  }

  .badge {
    font-size: 10px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 9999px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .badge-recommended {
    background: var(--success-fade);
    color: var(--success);
  }

  .badge-alt {
    background: var(--accent-fade);
    color: var(--accent);
  }

  .provider-bullets {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 16px;
  }

  .provider-bullets li {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
  }

  .provider-bullets li strong {
    color: var(--text-primary);
    font-weight: 500;
  }

  /* Key steps */
  .key-steps {
    background: var(--bg-subtle);
    border-radius: 12px;
    padding: 18px 22px;
    margin-bottom: 24px;
  }

  .key-steps ol {
    margin: 0;
    padding-left: 18px;
    color: var(--text-primary);
    font-size: 13px;
    line-height: 1.8;
  }

  .key-steps li code {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 11px;
  }

  .key-input-block {
    background: var(--bg-subtle);
    border-radius: 12px;
    padding: 18px;
    margin-bottom: 24px;
  }

  .key-input-block label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .key-input-block label span {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .key-input-block input {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 14px;
    color: var(--text-primary);
    font-family: ui-monospace, monospace;
    outline: none;
    transition: border-color 120ms ease, box-shadow 120ms ease;
  }

  .key-input-block input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-fade);
  }

  .status {
    margin-top: 12px;
    padding: 8px 14px;
    border-radius: 8px;
    font-size: 13px;
  }

  .status-ok {
    background: var(--success-fade);
    color: var(--success);
  }

  .status-error {
    background: var(--danger-fade);
    color: var(--danger);
  }

  .status-testing {
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  /* Hotkey list */
  .hotkey-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 24px;
  }

  .hotkey-card {
    background: var(--bg-subtle);
    border-radius: 12px;
    padding: 14px 16px;
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .hotkey-card kbd {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 6px;
    padding: 6px 12px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 13px;
    color: var(--text-primary);
    min-width: 36px;
    text-align: center;
    flex-shrink: 0;
  }

  .hotkey-text strong {
    font-size: 14px;
    color: var(--text-primary);
    display: block;
    margin-bottom: 2px;
  }

  .hotkey-text p {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 0;
    line-height: 1.5;
  }

  /* Actions */
  .actions {
    display: flex;
    gap: 10px;
    margin-top: 8px;
    align-items: center;
    flex-wrap: wrap;
  }

  .primary {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 8px;
    padding: 10px 20px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    transition: opacity 120ms ease;
  }

  .primary:hover:not(:disabled) {
    opacity: 0.92;
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .primary.big {
    padding: 12px 28px;
    font-size: 15px;
  }

  .primary.external {
    background: var(--bg-card);
    color: var(--accent);
    border: 1px solid var(--accent);
  }

  .ghost {
    background: transparent;
    color: var(--text-secondary);
    border: none;
    padding: 10px 14px;
    font-size: 13px;
    cursor: pointer;
    border-radius: 8px;
  }

  .ghost:hover {
    color: var(--text-primary);
    background: var(--bg-subtle);
  }

  kbd {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 1px 6px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 12px;
    color: var(--text-primary);
  }
</style>
