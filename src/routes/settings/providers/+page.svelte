<script lang="ts">
  // Providers & Models — STT/LLM provider config + API keys + per-mode
  // model picker, prompt editor, language hint. The biggest single page
  // in Settings; deliberately combines provider+model since you can't
  // meaningfully pick a model without picking a provider first.
  import { onMount } from "svelte";
  import { api, type SecretCheck, type SecretsDiagnostic } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";

  let secretCheck = $state<SecretCheck>({ stt: false, llm: false, gemini: false });
  let diag = $state<SecretsDiagnostic | null>(null);

  async function refreshDiagnostic() {
    try {
      diag = await api.secretsDiagnostic();
    } catch (e) {
      console.warn("secrets diagnostic failed", e);
    }
  }
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
  // Gemini model list — checked as of 1 June 2026.
  //
  // What changed since the last refresh:
  //   • 2.0 Flash and 2.0 Flash-Lite are deprecated on 1 June 2026 → removed.
  //   • 3.5 Flash launched at I/O on 20 May 2026, model id gemini-3.5-flash
  //     → added at top of the list (recommended default).
  //   • 2.5 Flash-Lite added — highest free quota (1,000 req/day, 15 RPM).
  //   • "gemini-3-pro" entry was speculative; replaced with 2.5 Pro which
  //     is the actual current Pro model (paid-only since Apr 2026).
  //   • 3.5 Pro is announced for GA in June 2026 but no API model id is
  //     published yet — will add when Google ships it.
  const GEMINI_LLM_MODELS: ModelOpt[] = [
    { id: "gemini-3.5-flash",      label: "Gemini 3.5 Flash",      quality: "Newest • Default since I/O 2026" },
    { id: "gemini-2.5-flash",      label: "Gemini 2.5 Flash",      quality: "Fast • 250/day free" },
    { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite", quality: "Fastest • 1,000/day free (highest quota)" },
    { id: "gemini-3-flash",        label: "Gemini 3 Flash",        quality: "Stable • Free tier" },
    { id: "gemini-2.5-pro",        label: "Gemini 2.5 Pro",        quality: "Smartest • PAID ONLY since Apr 2026" },
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
    // Persistence is automatic (settings.set writes through to disk + Rust),
    // but the user asked for explicit confirmation that the selection saved.
    // Reported as "should be a save button" — better resolved with a toast
    // than a literal save button, since auto-save is the right behaviour and
    // the toast confirms the write succeeded.
    flash(`Provider set to ${provider === "gemini" ? "Gemini" : "Groq"}`);
  }

  async function changeLlmModel(modelId: string) {
    await settings.set("llm_model", modelId as any);
    const label = modelsForProvider(settings.s.llm_provider)
      .find((m) => m.id === modelId)?.label ?? modelId;
    flash(`Model: ${label}`);
  }

  async function changeSttModel(modelId: string) {
    await settings.set("stt_model", modelId as any);
    const label = STT_MODELS.find((m) => m.id === modelId)?.label ?? modelId;
    flash(`STT: ${label}`);
  }

  // ── API key save / delete / test ───────────────────────────────────────
  async function saveStt() {
    if (!groqStt.trim()) return;
    saving = true;
    try {
      await api.saveSecret("groq_stt", groqStt.trim());
      groqStt = "";
      secretCheck = await api.checkSecrets();
      await refreshDiagnostic();
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
      await refreshDiagnostic();
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
      await refreshDiagnostic();
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

  onMount(async () => {
    secretCheck = await api.checkSecrets();
    await refreshDiagnostic();
  });

  function locationLabel(loc: "keyring" | "file" | "none"): string {
    return loc === "keyring"
      ? "OS keyring (secure)"
      : loc === "file"
      ? "File fallback (keyring unavailable)"
      : "Not saved";
  }
  function locationClass(loc: "keyring" | "file" | "none"): string {
    return loc === "keyring" ? "ok" : loc === "file" ? "warn" : "neutral";
  }
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

  <div class="settings-card">
    <h3>Speech-to-text</h3>
    <p class="lede">
      Which service transcribes your audio. Groq Whisper is wired in;
      others below are planned and selectable once their backend lands.
    </p>
    <div class="provider-model-row">
      <div class="field-block field-half">
        <label>Service</label>
        <select
          value={settings.s.stt_provider}
          onchange={(e) => settings.set("stt_provider", (e.currentTarget as HTMLSelectElement).value as any)}
        >
          <option value="groq">Groq Whisper</option>
          <option value="sarvam" disabled>Sarvam Saaras (Hindi / Indic — coming soon)</option>
          <option value="deepgram" disabled>Deepgram Nova-3 (coming soon)</option>
          <option value="assemblyai" disabled>AssemblyAI (coming soon)</option>
          <option value="gemini-stt" disabled>Gemini 2.5 Flash multimodal (coming soon)</option>
        </select>
      </div>
      <div class="field-block field-half">
        <label>Model</label>
        <select
          value={settings.s.stt_model}
          onchange={(e) => changeSttModel((e.currentTarget as HTMLSelectElement).value)}
        >
          {#each STT_MODELS as m (m.id)}
            <option value={m.id}>{m.label} — {m.quality}</option>
          {/each}
        </select>
      </div>
    </div>

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
  </div>

  <div class="settings-card">
    <h3>LLM cleanup</h3>
    <p class="lede">
      Used by F9 (and F8 if you've enabled cleanup for it). Same model handles all modes;
      only the prompt changes per mode. Saved keys persist when you switch providers.
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
          <option value="anthropic" disabled>Anthropic Claude (coming soon)</option>
          <option value="openai" disabled>OpenAI GPT (coming soon)</option>
          <option value="sarvam-m" disabled>Sarvam-M (Indic — coming soon)</option>
          <option value="openrouter" disabled>OpenRouter aggregator (coming soon)</option>
        </select>
      </div>
      <div class="field-block field-half">
        <label>Model</label>
        <select
          value={settings.s.llm_model}
          onchange={(e) => changeLlmModel((e.currentTarget as HTMLSelectElement).value)}
        >
          {#each modelsForProvider(settings.s.llm_provider) as m (m.id)}
            <option value={m.id}>{m.label} — {m.quality}</option>
          {/each}
        </select>
      </div>
    </div>
  </div>

  <p class="tip">
    Per-mode behaviour (cleanup toggle + custom system prompt) lives in
    <strong>Settings → Modes</strong>. Hotkey bindings live in
    <strong>Settings → Dictation</strong>.
  </p>

  <!-- ── Storage diagnostic ───────────────────────────────────────────── -->
  {#if diag}
    <div class="diag-card">
      <div class="diag-head">
        <h3>Key storage status</h3>
        <button class="btn-link" onclick={refreshDiagnostic}>Refresh</button>
      </div>
      <p class="lede">
        Where each saved key is currently stored. Keyring is preferred (encrypted by
        the OS — Windows Credential Manager, macOS Keychain). If the keyring isn't
        reliable on your machine, wispr-fox automatically falls back to a file at
        the path below — your keys still work, just stored in plaintext under your
        user profile.
      </p>
      <ul class="diag-list">
        <li>
          <span class="diag-label">Groq STT</span>
          <span class="diag-loc {locationClass(diag.stt)}">{locationLabel(diag.stt)}</span>
        </li>
        <li>
          <span class="diag-label">Groq LLM</span>
          <span class="diag-loc {locationClass(diag.llm)}">{locationLabel(diag.llm)}</span>
        </li>
        <li>
          <span class="diag-label">Gemini</span>
          <span class="diag-loc {locationClass(diag.gemini)}">{locationLabel(diag.gemini)}</span>
        </li>
      </ul>
      {#if !diag.keyring_works && (diag.stt !== "none" || diag.llm !== "none" || diag.gemini !== "none")}
        <p class="diag-warn">
          ⚠️ The OS keyring isn't accepting writes on this machine — your keys are
          stored in the file fallback. They still work; it's just slightly less
          secure than the keyring. Common causes: a corrupted Windows Credential
          Manager entry (try deleting any old <code>wispr-fox</code> entries from
          <code>Control Panel → Credential Manager</code> and saving again), or a
          macOS app without keychain entitlement.
        </p>
      {/if}
      <p class="diag-path">
        File fallback location: <code>{diag.fallback_path}</code>
        {diag.fallback_exists ? "(exists)" : "(empty)"}
      </p>
    </div>
  {/if}
</section>

<style>
  .tip {
    background: var(--bg-subtle);
    border: 1px dashed var(--border);
    border-radius: 10px;
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 24px 0 0;
    max-width: 720px;
  }

  .diag-card {
    margin: 28px 0 0;
    padding: 16px 18px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    max-width: 720px;
  }
  .diag-card h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .diag-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .btn-link {
    background: none;
    border: none;
    color: var(--accent, #ec7c34);
    font-size: 12px;
    cursor: pointer;
    padding: 2px 6px;
  }
  .btn-link:hover {
    text-decoration: underline;
  }
  .diag-list {
    list-style: none;
    padding: 0;
    margin: 12px 0 8px;
    display: grid;
    gap: 4px;
  }
  .diag-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 0;
    border-bottom: 1px dashed var(--border);
    font-size: 13px;
  }
  .diag-list li:last-child {
    border-bottom: none;
  }
  .diag-label {
    font-weight: 500;
    color: var(--text);
  }
  .diag-loc {
    font-size: 12px;
    padding: 3px 8px;
    border-radius: 999px;
    font-weight: 500;
  }
  .diag-loc.ok {
    background: rgba(76, 175, 80, 0.15);
    color: #2e7d32;
  }
  .diag-loc.warn {
    background: rgba(255, 152, 0, 0.18);
    color: #b06800;
  }
  .diag-loc.neutral {
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }
  .diag-warn {
    margin: 10px 0 6px;
    padding: 10px 12px;
    background: rgba(255, 152, 0, 0.1);
    border-left: 3px solid #ff9800;
    border-radius: 4px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text);
  }
  .diag-warn code {
    background: rgba(0, 0, 0, 0.07);
    padding: 1px 5px;
    border-radius: 3px;
    font-size: 11px;
  }
  .diag-path {
    margin: 6px 0 0;
    font-size: 11px;
    color: var(--text-secondary);
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    word-break: break-all;
  }
  .diag-path code {
    background: var(--bg-subtle);
    padding: 1px 4px;
    border-radius: 3px;
  }
</style>
