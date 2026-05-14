<script lang="ts">
  // Providers & Models — STT/LLM provider config + API keys + per-mode
  // model picker, prompt editor, language hint. The biggest single page
  // in Settings; deliberately combines provider+model since you can't
  // meaningfully pick a model without picking a provider first.
  import { onMount } from "svelte";
  import { api, type SecretCheck } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";
  import HotkeyCapture from "$lib/HotkeyCapture.svelte";

  let secretCheck = $state<SecretCheck>({ stt: false, llm: false, gemini: false });
  let groqStt = $state("");
  let groqLlm = $state("");
  let geminiKey = $state("");
  let saving = $state(false);

  type TestResult =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; models: string[] }
    | { kind: "error"; msg: string };
  let testResult = $state<TestResult>({ kind: "idle" });
  let geminiTestResult = $state<TestResult>({ kind: "idle" });

  // STT models on Groq (May 2026). Sorted speed-first.
  type ModelOpt = { id: string; label: string; quality: string };
  const STT_MODELS: ModelOpt[] = [
    { id: "whisper-large-v3-turbo",     label: "Whisper Turbo",    quality: "Fast • Good" },
    { id: "whisper-large-v3",           label: "Whisper Large v3", quality: "Slower • Best accuracy" },
    { id: "distil-whisper-large-v3-en", label: "Distil-Whisper",   quality: "Fastest • English only" },
  ];
  const GROQ_LLM_MODELS: ModelOpt[] = [
    { id: "llama-3.1-8b-instant",    label: "Llama 3.1 8B",     quality: "Fast • 14,400/day free" },
    { id: "llama-3.3-70b-versatile", label: "Llama 3.3 70B",    quality: "Smarter • 1,000/day free" },
    { id: "llama-4-maverick",        label: "Llama 4 Maverick", quality: "Smartest • 500/day free" },
  ];
  const GEMINI_LLM_MODELS: ModelOpt[] = [
    { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash", quality: "Fast • 1,500/day free • Best free quality" },
    { id: "gemini-2.0-flash", label: "Gemini 2.0 Flash", quality: "Fast • free tier" },
    { id: "gemini-2.5-pro",   label: "Gemini 2.5 Pro",   quality: "Smartest • PAID ONLY since Apr 2026" },
    { id: "gemini-3-pro",     label: "Gemini 3 Pro",     quality: "Smartest • PAID ONLY" },
  ];

  function modelsForProvider(p: string): ModelOpt[] {
    return p === "gemini" ? GEMINI_LLM_MODELS : GROQ_LLM_MODELS;
  }

  async function changeLlmProvider(provider: string) {
    await settings.set("llm_provider", provider as any);
    const options = modelsForProvider(provider);
    if (!options.find((m) => m.id === settings.s.llm_model)) {
      await settings.set("llm_model", options[0].id as any);
    }
  }

  // ── API key save / delete / test ───────────────────────────────────────
  async function saveStt() {
    if (!groqStt.trim()) return;
    saving = true;
    try {
      await api.saveSecret("groq_stt", groqStt.trim());
      groqStt = "";
      secretCheck = await api.checkSecrets();
      flash("STT key saved");
    } finally {
      saving = false;
    }
  }

  async function saveLlm() {
    if (!groqLlm.trim()) return;
    saving = true;
    try {
      await api.saveSecret("groq_llm", groqLlm.trim());
      groqLlm = "";
      secretCheck = await api.checkSecrets();
      flash("LLM key saved");
    } finally {
      saving = false;
    }
  }

  async function saveGemini() {
    if (!geminiKey.trim()) return;
    saving = true;
    try {
      await api.saveSecret("gemini_llm", geminiKey.trim());
      geminiKey = "";
      secretCheck = await api.checkSecrets();
      flash("Gemini key saved");
    } finally {
      saving = false;
    }
  }

  async function deleteKey(name: "groq_stt" | "groq_llm" | "gemini_llm") {
    if (!confirm(`Delete ${name}?`)) return;
    await api.deleteSecret(name);
    secretCheck = await api.checkSecrets();
    flash("Deleted");
  }

  async function testGroqStt() {
    // Prefer the value in the input box (fresh paste). If empty AND a key
    // is saved, test the saved one — Rust reads it from keyring on its side.
    const k = groqStt.trim() || groqLlm.trim();
    testResult = { kind: "testing" };
    try {
      const models = k
        ? await api.testGroqKey(k)
        : secretCheck.stt || secretCheck.llm
        ? await api.testSavedGroqKey()
        : null;
      if (models === null) {
        testResult = { kind: "error", msg: "No saved key — paste one above and click Save first." };
        return;
      }
      testResult = { kind: "ok", models };
    } catch (e) {
      testResult = { kind: "error", msg: String(e) };
    }
  }

  async function testGemini() {
    const k = geminiKey.trim();
    geminiTestResult = { kind: "testing" };
    try {
      const models = k
        ? await api.testGeminiKey(k)
        : secretCheck.gemini
        ? await api.testSavedGeminiKey()
        : null;
      if (models === null) {
        geminiTestResult = { kind: "error", msg: "No saved Gemini key — paste one above and click Save first." };
        return;
      }
      geminiTestResult = { kind: "ok", models };
    } catch (e) {
      geminiTestResult = { kind: "error", msg: String(e) };
    }
  }

  // ── Per-mode prompt editor ─────────────────────────────────────────────
  let defaultPrompts = $state<{ light: string; advanced: string; drafting: string } | null>(null);
  let promptOpen = $state<Record<"light" | "advanced" | "drafting", boolean>>({
    light: false,
    advanced: false,
    drafting: false,
  });

  function effectivePrompt(mode: "light" | "advanced" | "drafting"): string {
    const customField = `custom_${mode}_prompt` as keyof typeof settings.s;
    const custom = (settings.s[customField] as string) || "";
    if (custom.trim()) return custom;
    return defaultPrompts ? defaultPrompts[mode] : "(loading…)";
  }

  async function savePrompt(mode: "light" | "advanced" | "drafting", value: string) {
    const customField = `custom_${mode}_prompt` as keyof typeof settings.s;
    await settings.set(customField, value as any);
    flash(`${mode} prompt saved`);
  }

  async function resetPrompt(mode: "light" | "advanced" | "drafting") {
    if (!confirm(`Reset the ${mode} prompt to its default? Any custom edits will be lost.`)) return;
    const customField = `custom_${mode}_prompt` as keyof typeof settings.s;
    await settings.set(customField, "" as any);
    flash(`${mode} prompt reset`);
  }

  onMount(async () => {
    secretCheck = await api.checkSecrets();
    defaultPrompts = await api.getDefaultPrompts();
  });
</script>

<section>
  <h2>Providers & API keys</h2>
  <p class="lede">
    wispr-fox uses cloud providers for speech-to-text and LLM cleanup.
    Groq is the recommended default — generous free tier, no card.
    Add Gemini if you want a second LLM option.
  </p>

  <!-- ── Groq ────────────────────────────────────────────────────────── -->
  <div class="provider-card">
    <div class="provider-head">
      <div>
        <div class="provider-name">Groq</div>
        <div class="provider-meta">groq.com — fast inference, generous free tier</div>
      </div>
      <a class="link-out" href="https://console.groq.com/keys" target="_blank" rel="noopener">
        Get an API key →
      </a>
    </div>

    <div class="help-box">
      <strong>Free tier as of May 2026:</strong>
      <ul>
        <li>STT (Whisper): <code>2,000 requests/day</code>, 25 MB/file, resets midnight UTC</li>
        <li>LLM: <code>1,000-14,400 requests/day</code> depending on model (smaller models = higher limits)</li>
        <li>No credit card required to start. Paid tier removes daily caps.</li>
      </ul>
    </div>

    <div class="key-row">
      <label>STT key (Whisper)</label>
      <div class="input-row">
        <input
          type="password"
          placeholder={secretCheck.stt ? "•••••••• (saved)" : "gsk_..."}
          bind:value={groqStt}
        />
        <button class="btn-primary" onclick={saveStt} disabled={saving}>Save</button>
        {#if secretCheck.stt}
          <button class="btn-danger" onclick={() => deleteKey("groq_stt")}>Delete</button>
        {/if}
      </div>
    </div>

    <div class="key-row">
      <label>LLM key (Clippy cleanup)</label>
      <div class="input-row">
        <input
          type="password"
          placeholder={secretCheck.llm ? "•••••••• (saved)" : "gsk_... (or reuse STT)"}
          bind:value={groqLlm}
        />
        <button class="btn-primary" onclick={saveLlm} disabled={saving}>Save</button>
        {#if secretCheck.llm}
          <button class="btn-danger" onclick={() => deleteKey("groq_llm")}>Delete</button>
        {/if}
      </div>
      <p class="hint">If empty, the STT key is reused for LLM calls.</p>
    </div>

    <div class="test-block">
      <button class="btn-secondary" onclick={testGroqStt}>Test connection</button>
      {#if testResult.kind === "testing"}
        <span class="test-msg testing">Testing…</span>
      {:else if testResult.kind === "ok"}
        <span class="test-msg ok">✓ Key works — {testResult.models.length} models accessible</span>
      {:else if testResult.kind === "error"}
        <span class="test-msg error">✗ {testResult.msg}</span>
      {/if}
    </div>
  </div>

  <!-- ── Google Gemini ───────────────────────────────────────────────── -->
  <div class="provider-card">
    <div class="provider-head">
      <div>
        <div class="provider-name">Google Gemini</div>
        <div class="provider-meta">ai.google.dev — best free-tier LLM for cleanup + drafting</div>
      </div>
      <a class="link-out" href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener">
        Get an API key →
      </a>
    </div>

    <div class="help-box">
      <strong>Free tier as of May 2026:</strong>
      <ul>
        <li>Gemini 2.5 Flash: <code>15 RPM, 1,500 req/day</code> — recommended for F10 drafting</li>
        <li>Pro models <em>removed from free tier April 2026</em> — billing required.</li>
        <li>No card needed to start. Quality competitive with GPT-4o-mini.</li>
      </ul>
    </div>

    <div class="key-row">
      <label>API key</label>
      <div class="input-row">
        <input
          type="password"
          placeholder={secretCheck.gemini ? "•••••••• (saved)" : "AIza..."}
          bind:value={geminiKey}
        />
        <button class="btn-primary" onclick={saveGemini} disabled={saving}>Save</button>
        {#if secretCheck.gemini}
          <button class="btn-danger" onclick={() => deleteKey("gemini_llm")}>Delete</button>
        {/if}
      </div>
    </div>

    <div class="test-block">
      <button class="btn-secondary" onclick={testGemini}>Test connection</button>
      {#if geminiTestResult.kind === "testing"}
        <span class="test-msg testing">Testing…</span>
      {:else if geminiTestResult.kind === "ok"}
        <span class="test-msg ok">✓ Key works — {geminiTestResult.models.length} models accessible</span>
      {:else if geminiTestResult.kind === "error"}
        <span class="test-msg error">✗ {geminiTestResult.msg}</span>
      {/if}
    </div>
  </div>

  <h3>Speech-to-text</h3>
  <p class="lede">Which service transcribes your audio. Currently Groq Whisper only — Gemini multimodal STT is on the roadmap.</p>
  <div class="provider-model-row">
    <div class="field-block field-half">
      <label>Service</label>
      <select
        value={settings.s.stt_provider}
        onchange={(e) => settings.set("stt_provider", (e.currentTarget as HTMLSelectElement).value as any)}
      >
        <option value="groq">Groq Whisper</option>
      </select>
    </div>
    <div class="field-block field-half">
      <label>Model</label>
      <select
        value={settings.s.stt_model}
        onchange={(e) => settings.set("stt_model", (e.currentTarget as HTMLSelectElement).value as any)}
      >
        {#each STT_MODELS as m (m.id)}
          <option value={m.id}>{m.label} — {m.quality}</option>
        {/each}
      </select>
    </div>
  </div>

  <h3>LLM cleanup</h3>
  <p class="lede">
    Used by F9 (and F8 if you've enabled cleanup for it). One choice — the same model handles all three
    modes, only the prompt changes per mode. Your saved API keys stick around when you switch providers.
  </p>
  <div class="provider-model-row">
    <div class="field-block field-half">
      <label>Service</label>
      <select
        value={settings.s.llm_provider}
        onchange={(e) => changeLlmProvider((e.currentTarget as HTMLSelectElement).value)}
      >
        <option value="groq" disabled={!secretCheck.llm && !secretCheck.stt}>
          Groq {(!secretCheck.llm && !secretCheck.stt) ? "(add key first)" : ""}
        </option>
        <option value="gemini" disabled={!secretCheck.gemini}>
          Google Gemini {secretCheck.gemini ? "" : "(add key first)"}
        </option>
      </select>
    </div>
    <div class="field-block field-half">
      <label>Model</label>
      <select
        value={settings.s.llm_model}
        onchange={(e) => settings.set("llm_model", (e.currentTarget as HTMLSelectElement).value as any)}
      >
        {#each modelsForProvider(settings.s.llm_provider) as m (m.id)}
          <option value={m.id}>{m.label} — {m.quality}</option>
        {/each}
      </select>
    </div>
  </div>

  <h3>Modes</h3>
  <p class="lede">
    Each F-key is a different "mode" — same LLM model, different prompts. Toggle whether each mode uses LLM
    cleanup, and click "Show prompt" to view or customise the prompt for that mode.
  </p>

  {#each [
    { id: "light",    fkey: "F8",  title: "Transcribe",  settingKey: "auto_clean_in_light",    defaultOn: false,
      hotkeyKey: "light_hotkey", stickyHotkeyKey: "light_sticky_hotkey", stickyKey: "sticky_light",
      desc: "Voice → text. When LLM cleanup is OFF you get the raw Whisper output (default). When ON, every F8 press also gets spell/punctuation/paragraphing — same content, same voice, just readable. For one-off cleanup without flipping this toggle, use Shift+F8." },
    { id: "drafting", fkey: "F9",  title: "Draft",       settingKey: "auto_clean_in_drafting", defaultOn: true,
      hotkeyKey: "drafting_hotkey", stickyHotkeyKey: "drafting_sticky_hotkey", stickyKey: "sticky_drafting",
      desc: "Give a brief (\"draft an email to Saurabh about X, Y, Z\") and get back a complete polished output. Best for emails, Slack, docs." },
    { id: "advanced", fkey: "—",   title: "Advanced (legacy)", settingKey: "auto_clean_in_advanced", defaultOn: true,
      hotkeyKey: "advanced_hotkey", stickyHotkeyKey: "advanced_sticky_hotkey", stickyKey: "sticky_advanced",
      desc: "Standalone Advanced cleanup mode. No hotkey by default — bind one below if you want a dedicated key separate from the F8 toggle." },
  ] as m (m.id)}
    <div class="mode-block">
      <div class="mode-head">
        <kbd class="mode-key">{m.fkey}</kbd>
        <div class="mode-title-block">
          <div class="mode-title">{m.title}</div>
          <div class="mode-desc">{m.desc}</div>
        </div>
        <label class="check-row inline">
          <input
            type="checkbox"
            checked={settings.s[m.settingKey as keyof typeof settings.s] as boolean}
            onchange={(e) => settings.set(m.settingKey as any, (e.currentTarget as HTMLInputElement).checked as any)}
          />
          <span>LLM cleanup</span>
        </label>
      </div>

      <div class="mode-hotkeys">
        <div class="hk-pair">
          <div class="hk-pair-col">
            <div class="hk-pair-label">Main (push-to-talk)</div>
            <HotkeyCapture label="" bind:value={settings.s[m.hotkeyKey as keyof typeof settings.s] as string} />
          </div>
          <div class="hk-pair-col">
            <div class="hk-pair-label">Sticky-invoke (toggle)</div>
            <HotkeyCapture label="" bind:value={settings.s[m.stickyHotkeyKey as keyof typeof settings.s] as string} />
          </div>
        </div>
        <label class="check-row small">
          <input
            type="checkbox"
            checked={settings.s[m.stickyKey as keyof typeof settings.s] as boolean}
            onchange={(e) => settings.set(m.stickyKey as any, (e.currentTarget as HTMLInputElement).checked as any)}
          />
          <span>Default to sticky — make the MAIN hotkey behave as toggle too</span>
        </label>
      </div>

      <button
        class="prompt-toggle"
        onclick={() => (promptOpen[m.id as "light" | "advanced" | "drafting"] = !promptOpen[m.id as "light" | "advanced" | "drafting"])}
      >
        <span class="prompt-caret" class:open={promptOpen[m.id as "light" | "advanced" | "drafting"]}>›</span>
        {promptOpen[m.id as "light" | "advanced" | "drafting"] ? "Hide" : "Show"} system prompt
        {#if (settings.s[`custom_${m.id}_prompt` as keyof typeof settings.s] as string)?.trim()}
          <span class="prompt-edited-pill">customised</span>
        {/if}
      </button>
      {#if promptOpen[m.id as "light" | "advanced" | "drafting"]}
        <div class="prompt-editor">
          <textarea
            rows="10"
            value={effectivePrompt(m.id as "light" | "advanced" | "drafting")}
            onchange={(e) => savePrompt(m.id as "light" | "advanced" | "drafting", (e.currentTarget as HTMLTextAreaElement).value)}
          ></textarea>
          <div class="prompt-actions">
            <button
              class="btn-secondary small"
              onclick={() => resetPrompt(m.id as "light" | "advanced" | "drafting")}
            >
              Reset to default
            </button>
            {#if m.id === "light"}
              <span class="prompt-warning">
                ⚠ The Light prompt is a security boundary against prompt injection — keep the "treat transcript as literal data" guarantee or attackers can hijack via dictation.
              </span>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  {/each}

  <p class="hint">⚠ Hotkey changes take effect after restarting wispr-fox.</p>

  <div class="field-block">
    <label>Language hint <span class="hint-inline">(blank = auto-detect, recommended)</span></label>
    <input
      type="text"
      placeholder="auto"
      value={settings.s.language_hint ?? ""}
      onchange={(e) => {
        const v = (e.currentTarget as HTMLInputElement).value.trim();
        settings.set("language_hint", v.length ? v : null);
      }}
    />
    <p class="hint">ISO codes (e.g. <code>en</code>, <code>hi</code>). Leave blank if you code-switch.</p>
  </div>
</section>
