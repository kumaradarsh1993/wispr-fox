<script lang="ts">
  import { onMount } from "svelte";
  import { api, type SecretCheck, type SecretKeyName, type SecretsDiagnostic } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";

  type TestResult =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; models: string[] }
    | { kind: "error"; msg: string };
  type ModelOpt = { id: string; label: string; quality: string };

  const EMPTY_SECRETS: SecretCheck = {
    stt: false,
    llm: false,
    gemini: false,
    openai_stt: false,
    openai_llm: false,
    deepgram_stt: false,
    elevenlabs_stt: false,
    any_stt: false,
  };

  let secretCheck = $state<SecretCheck>({ ...EMPTY_SECRETS });
  let diag = $state<SecretsDiagnostic | null>(null);
  let saving = $state(false);

  let groqStt = $state("");
  let groqLlm = $state("");
  let openaiStt = $state("");
  let openaiLlm = $state("");
  let deepgramKey = $state("");
  let elevenlabsKey = $state("");
  let geminiKey = $state("");

  let groqTestResult = $state<TestResult>({ kind: "idle" });
  let openaiTestResult = $state<TestResult>({ kind: "idle" });
  let deepgramTestResult = $state<TestResult>({ kind: "idle" });
  let elevenlabsTestResult = $state<TestResult>({ kind: "idle" });
  let geminiTestResult = $state<TestResult>({ kind: "idle" });

  const STT_MODELS: Record<string, ModelOpt[]> = {
    groq: [
      { id: "whisper-large-v3-turbo", label: "Whisper Turbo", quality: "Fast, strong default" },
      { id: "whisper-large-v3", label: "Whisper Large v3", quality: "Slower, highest Whisper accuracy" },
      { id: "distil-whisper-large-v3-en", label: "Distil-Whisper", quality: "Fastest, English only" },
    ],
    openai: [
      { id: "gpt-4o-transcribe", label: "GPT-4o Transcribe", quality: "Best OpenAI STT quality" },
      { id: "gpt-4o-mini-transcribe", label: "GPT-4o mini Transcribe", quality: "Lower cost, fast" },
      { id: "whisper-1", label: "Whisper API", quality: "Legacy compatible fallback" },
    ],
    deepgram: [
      { id: "nova-3", label: "Nova-3", quality: "Recommended Deepgram v2 batch model" },
      { id: "nova-2", label: "Nova-2", quality: "Stable fallback" },
    ],
    elevenlabs: [
      { id: "scribe_v2", label: "Scribe v2", quality: "Recommended ElevenLabs STT" },
      { id: "scribe_v1", label: "Scribe v1", quality: "Legacy fallback" },
    ],
  };

  const LLM_MODELS: Record<string, ModelOpt[]> = {
    groq: [
      { id: "llama-3.3-70b-versatile", label: "Llama 3.3 70B", quality: "Balanced default" },
      { id: "llama-3.1-8b-instant", label: "Llama 3.1 8B", quality: "Fastest cleanup" },
      { id: "llama-4-maverick", label: "Llama 4 Maverick", quality: "Higher quality, lower free quota" },
    ],
    gemini: [
      { id: "gemini-3.5-flash", label: "Gemini 3.5 Flash", quality: "Current default" },
      { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash", quality: "Fast free-tier option" },
      { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite", quality: "Lowest latency / quota friendly" },
      { id: "gemini-3-flash-preview", label: "Gemini 3 Flash Preview", quality: "Preview, use only if your key lists it" },
      { id: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro Preview", quality: "Preview Pro, billing likely required" },
      { id: "gemini-2.5-pro", label: "Gemini 2.5 Pro", quality: "Paid tier" },
    ],
    openai: [
      { id: "gpt-5.4-mini", label: "GPT-5.4 mini", quality: "Fast OpenAI cleanup default" },
      { id: "gpt-5.4", label: "GPT-5.4", quality: "Higher quality" },
      { id: "gpt-5.5", label: "GPT-5.5", quality: "Frontier quality, slower/costlier" },
    ],
  };

  function providerLabel(provider: string): string {
    return provider === "groq"
      ? "Groq"
      : provider === "openai"
      ? "OpenAI"
      : provider === "deepgram"
      ? "Deepgram"
      : provider === "elevenlabs"
      ? "ElevenLabs"
      : provider === "gemini"
      ? "Google Gemini"
      : provider;
  }

  function sttReady(provider: string): boolean {
    return provider === "groq"
      ? secretCheck.stt
      : provider === "openai"
      ? secretCheck.openai_stt || secretCheck.openai_llm
      : provider === "deepgram"
      ? secretCheck.deepgram_stt
      : provider === "elevenlabs"
      ? secretCheck.elevenlabs_stt
      : false;
  }

  function llmReady(provider: string): boolean {
    return provider === "groq"
      ? secretCheck.llm || secretCheck.stt
      : provider === "gemini"
      ? Boolean(secretCheck.gemini)
      : provider === "openai"
      ? secretCheck.openai_llm || secretCheck.openai_stt
      : false;
  }

  function sttModelsFor(provider: string): ModelOpt[] {
    return STT_MODELS[provider] ?? STT_MODELS.groq;
  }

  function llmModelsFor(provider: string): ModelOpt[] {
    return LLM_MODELS[provider] ?? LLM_MODELS.groq;
  }

  async function refreshSecrets() {
    secretCheck = await api.checkSecrets();
    await refreshDiagnostic();
  }

  async function refreshDiagnostic() {
    try {
      diag = await api.secretsDiagnostic();
    } catch (e) {
      console.warn("secrets diagnostic failed", e);
    }
  }

  async function saveKey(key: SecretKeyName, value: string, clear: () => void, label: string) {
    if (!value.trim()) return;
    saving = true;
    try {
      await api.saveSecret(key, value.trim());
      clear();
      await refreshSecrets();
      flash(`${label} key saved`);
    } finally {
      saving = false;
    }
  }

  async function deleteKey(name: SecretKeyName) {
    if (!confirm(`Delete ${name}?`)) return;
    await api.deleteSecret(name);
    await refreshSecrets();
    flash("Deleted");
  }

  async function runTest(
    freshKey: string,
    hasSaved: boolean,
    setResult: (r: TestResult) => void,
    testFresh: (key: string) => Promise<string[]>,
    testSaved: () => Promise<string[]>,
    missing: string,
  ) {
    setResult({ kind: "testing" });
    try {
      const models = freshKey.trim() ? await testFresh(freshKey.trim()) : hasSaved ? await testSaved() : null;
      setResult(models ? { kind: "ok", models } : { kind: "error", msg: missing });
    } catch (e) {
      setResult({ kind: "error", msg: String(e) });
    }
  }

  async function changeSttProvider(provider: string) {
    await settings.set("stt_provider", provider as any);
    const options = sttModelsFor(provider);
    if (!options.find((m) => m.id === settings.s.stt_model)) {
      await settings.set("stt_model", options[0].id as any);
    }
    flash(`STT provider: ${providerLabel(provider)}`);
  }

  async function changeSttModel(modelId: string) {
    await settings.set("stt_model", modelId as any);
    const label = sttModelsFor(settings.s.stt_provider).find((m) => m.id === modelId)?.label ?? modelId;
    flash(`STT: ${label}`);
  }

  async function changeLlmProvider(provider: string) {
    await settings.set("llm_provider", provider as any);
    const options = llmModelsFor(provider);
    if (!options.find((m) => m.id === settings.s.llm_model)) {
      await settings.set("llm_model", options[0].id as any);
    }
    flash(`Cleanup provider: ${providerLabel(provider)}`);
  }

  async function changeLlmModel(modelId: string) {
    await settings.set("llm_model", modelId as any);
    const label = llmModelsFor(settings.s.llm_provider).find((m) => m.id === modelId)?.label ?? modelId;
    flash(`Cleanup model: ${label}`);
  }

  function locationLabel(loc: "keyring" | "file" | "none"): string {
    return loc === "keyring"
      ? "OS keyring"
      : loc === "file"
      ? "File fallback"
      : "Not saved";
  }

  function locationClass(loc: "keyring" | "file" | "none"): string {
    return loc === "keyring" ? "ok" : loc === "file" ? "warn" : "neutral";
  }

  function anyDiagSaved(d: SecretsDiagnostic): boolean {
    return [
      d.stt,
      d.llm,
      d.gemini,
      d.openai_stt,
      d.openai_llm,
      d.deepgram_stt,
      d.elevenlabs_stt,
    ].some((loc) => loc !== "none");
  }

  onMount(() => {
    refreshSecrets();
  });
</script>

<section>
  <h2>Providers & API keys</h2>
  <p class="lede">
    Choose who transcribes your audio and who handles cleanup. Keys are saved
    locally in the OS keyring first, with file fallback only when the keyring fails.
  </p>

  <div class="provider-grid">
    <div class="provider-card">
      <div class="provider-head">
        <div>
          <div class="provider-name">Groq</div>
          <div class="provider-meta">Whisper STT and Llama cleanup</div>
        </div>
        <a class="link-out" href="https://console.groq.com/keys" target="_blank" rel="noopener">Get key</a>
      </div>

      <div class="key-row">
        <label for="groq-stt-key">STT key</label>
        <div class="input-row">
          <input id="groq-stt-key" type="password" placeholder={secretCheck.stt ? "saved" : "gsk_..."} bind:value={groqStt} />
          <button class="btn-primary" onclick={() => saveKey("groq_stt", groqStt, () => (groqStt = ""), "Groq STT")} disabled={saving}>Save</button>
          {#if secretCheck.stt}<button class="btn-danger" onclick={() => deleteKey("groq_stt")}>Delete</button>{/if}
        </div>
      </div>

      <div class="key-row">
        <label for="groq-cleanup-key">Cleanup key</label>
        <div class="input-row">
          <input id="groq-cleanup-key" type="password" placeholder={secretCheck.llm ? "saved" : "gsk_... (or reuse STT)"} bind:value={groqLlm} />
          <button class="btn-primary" onclick={() => saveKey("groq_llm", groqLlm, () => (groqLlm = ""), "Groq cleanup")} disabled={saving}>Save</button>
          {#if secretCheck.llm}<button class="btn-danger" onclick={() => deleteKey("groq_llm")}>Delete</button>{/if}
        </div>
        <p class="hint">If no cleanup key is saved, the Groq STT key is reused.</p>
      </div>

      <div class="test-block">
        <button
          class="btn-secondary"
          onclick={() => runTest(groqStt || groqLlm, secretCheck.stt || secretCheck.llm, (r) => (groqTestResult = r), api.testGroqKey, api.testSavedGroqKey, "No saved Groq key.")}
        >Test connection</button>
        {#if groqTestResult.kind === "testing"}<span class="test-msg testing">Testing...</span>{/if}
        {#if groqTestResult.kind === "ok"}<span class="test-msg ok">Key works - {groqTestResult.models.length} models</span>{/if}
        {#if groqTestResult.kind === "error"}<span class="test-msg error">{groqTestResult.msg}</span>{/if}
      </div>
    </div>

    <div class="provider-card">
      <div class="provider-head">
        <div>
          <div class="provider-name">OpenAI</div>
          <div class="provider-meta">GPT transcription and cleanup</div>
        </div>
        <a class="link-out" href="https://platform.openai.com/api-keys" target="_blank" rel="noopener">Get key</a>
      </div>

      <div class="key-row">
        <label for="openai-stt-key">STT key</label>
        <div class="input-row">
          <input id="openai-stt-key" type="password" placeholder={secretCheck.openai_stt ? "saved" : "sk-..."} bind:value={openaiStt} />
          <button class="btn-primary" onclick={() => saveKey("openai_stt", openaiStt, () => (openaiStt = ""), "OpenAI STT")} disabled={saving}>Save</button>
          {#if secretCheck.openai_stt}<button class="btn-danger" onclick={() => deleteKey("openai_stt")}>Delete</button>{/if}
        </div>
      </div>

      <div class="key-row">
        <label for="openai-cleanup-key">Cleanup key</label>
        <div class="input-row">
          <input id="openai-cleanup-key" type="password" placeholder={secretCheck.openai_llm ? "saved" : "sk-... (or reuse STT)"} bind:value={openaiLlm} />
          <button class="btn-primary" onclick={() => saveKey("openai_llm", openaiLlm, () => (openaiLlm = ""), "OpenAI cleanup")} disabled={saving}>Save</button>
          {#if secretCheck.openai_llm}<button class="btn-danger" onclick={() => deleteKey("openai_llm")}>Delete</button>{/if}
        </div>
      </div>

      <div class="test-block">
        <button
          class="btn-secondary"
          onclick={() => runTest(openaiStt || openaiLlm, secretCheck.openai_stt || secretCheck.openai_llm, (r) => (openaiTestResult = r), api.testOpenAiKey, api.testSavedOpenAiKey, "No saved OpenAI key.")}
        >Test connection</button>
        {#if openaiTestResult.kind === "testing"}<span class="test-msg testing">Testing...</span>{/if}
        {#if openaiTestResult.kind === "ok"}<span class="test-msg ok">Key works - {openaiTestResult.models.length} models</span>{/if}
        {#if openaiTestResult.kind === "error"}<span class="test-msg error">{openaiTestResult.msg}</span>{/if}
      </div>
    </div>

    <div class="provider-card">
      <div class="provider-head">
        <div>
          <div class="provider-name">Deepgram</div>
          <div class="provider-meta">Nova transcription</div>
        </div>
        <a class="link-out" href="https://console.deepgram.com/" target="_blank" rel="noopener">Get key</a>
      </div>
      <div class="key-row">
        <label for="deepgram-stt-key">STT key</label>
        <div class="input-row">
          <input id="deepgram-stt-key" type="password" placeholder={secretCheck.deepgram_stt ? "saved" : "Deepgram token"} bind:value={deepgramKey} />
          <button class="btn-primary" onclick={() => saveKey("deepgram_stt", deepgramKey, () => (deepgramKey = ""), "Deepgram")} disabled={saving}>Save</button>
          {#if secretCheck.deepgram_stt}<button class="btn-danger" onclick={() => deleteKey("deepgram_stt")}>Delete</button>{/if}
        </div>
      </div>
      <div class="test-block">
        <button
          class="btn-secondary"
          onclick={() => runTest(deepgramKey, secretCheck.deepgram_stt, (r) => (deepgramTestResult = r), api.testDeepgramKey, api.testSavedDeepgramKey, "No saved Deepgram key.")}
        >Test connection</button>
        {#if deepgramTestResult.kind === "testing"}<span class="test-msg testing">Testing...</span>{/if}
        {#if deepgramTestResult.kind === "ok"}<span class="test-msg ok">Key works</span>{/if}
        {#if deepgramTestResult.kind === "error"}<span class="test-msg error">{deepgramTestResult.msg}</span>{/if}
      </div>
    </div>

    <div class="provider-card">
      <div class="provider-head">
        <div>
          <div class="provider-name">ElevenLabs</div>
          <div class="provider-meta">Scribe transcription</div>
        </div>
        <a class="link-out" href="https://elevenlabs.io/app/settings/api-keys" target="_blank" rel="noopener">Get key</a>
      </div>
      <div class="key-row">
        <label for="elevenlabs-stt-key">STT key</label>
        <div class="input-row">
          <input id="elevenlabs-stt-key" type="password" placeholder={secretCheck.elevenlabs_stt ? "saved" : "xi-api-key"} bind:value={elevenlabsKey} />
          <button class="btn-primary" onclick={() => saveKey("elevenlabs_stt", elevenlabsKey, () => (elevenlabsKey = ""), "ElevenLabs")} disabled={saving}>Save</button>
          {#if secretCheck.elevenlabs_stt}<button class="btn-danger" onclick={() => deleteKey("elevenlabs_stt")}>Delete</button>{/if}
        </div>
      </div>
      <div class="test-block">
        <button
          class="btn-secondary"
          onclick={() => runTest(elevenlabsKey, secretCheck.elevenlabs_stt, (r) => (elevenlabsTestResult = r), api.testElevenLabsKey, api.testSavedElevenLabsKey, "No saved ElevenLabs key.")}
        >Test connection</button>
        {#if elevenlabsTestResult.kind === "testing"}<span class="test-msg testing">Testing...</span>{/if}
        {#if elevenlabsTestResult.kind === "ok"}<span class="test-msg ok">Key works</span>{/if}
        {#if elevenlabsTestResult.kind === "error"}<span class="test-msg error">{elevenlabsTestResult.msg}</span>{/if}
      </div>
    </div>

    <div class="provider-card">
      <div class="provider-head">
        <div>
          <div class="provider-name">Google Gemini</div>
          <div class="provider-meta">Cleanup and drafting</div>
        </div>
        <a class="link-out" href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener">Get key</a>
      </div>
      <div class="key-row">
        <label for="gemini-api-key">API key</label>
        <div class="input-row">
          <input id="gemini-api-key" type="password" placeholder={secretCheck.gemini ? "saved" : "AIza..."} bind:value={geminiKey} />
          <button class="btn-primary" onclick={() => saveKey("gemini_llm", geminiKey, () => (geminiKey = ""), "Gemini")} disabled={saving}>Save</button>
          {#if secretCheck.gemini}<button class="btn-danger" onclick={() => deleteKey("gemini_llm")}>Delete</button>{/if}
        </div>
      </div>
      <div class="test-block">
        <button
          class="btn-secondary"
          onclick={() => runTest(geminiKey, Boolean(secretCheck.gemini), (r) => (geminiTestResult = r), api.testGeminiKey, api.testSavedGeminiKey, "No saved Gemini key.")}
        >Test connection</button>
        {#if geminiTestResult.kind === "testing"}<span class="test-msg testing">Testing...</span>{/if}
        {#if geminiTestResult.kind === "ok"}<span class="test-msg ok">Key works - {geminiTestResult.models.length} models</span>{/if}
        {#if geminiTestResult.kind === "error"}<span class="test-msg error">{geminiTestResult.msg}</span>{/if}
      </div>
    </div>
  </div>

  <div class="settings-card">
    <h3>Speech-to-text</h3>
    <div class="provider-model-row">
      <div class="field-block field-half">
        <label for="stt-provider">Service</label>
        <select id="stt-provider" value={settings.s.stt_provider} onchange={(e) => changeSttProvider((e.currentTarget as HTMLSelectElement).value)}>
          {#each Object.keys(STT_MODELS) as provider}
            <option value={provider} disabled={!sttReady(provider)}>
              {providerLabel(provider)} {sttReady(provider) ? "" : "(add key first)"}
            </option>
          {/each}
        </select>
      </div>
      <div class="field-block field-half">
        <label for="stt-model">Model</label>
        <select id="stt-model" value={settings.s.stt_model} onchange={(e) => changeSttModel((e.currentTarget as HTMLSelectElement).value)}>
          {#each sttModelsFor(settings.s.stt_provider) as m (m.id)}
            <option value={m.id}>{m.label} - {m.quality}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="field-block">
      <label for="language-hint">Language hint <span class="hint-inline">(blank = auto-detect)</span></label>
      <input
        id="language-hint"
        type="text"
        placeholder="auto"
        value={settings.s.language_hint ?? ""}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value.trim();
          settings.set("language_hint", v.length ? v : null);
        }}
      />
      <p class="hint">Use ISO codes like <code>en</code> or <code>hi</code>. Leave blank for code-switching.</p>
    </div>
  </div>

  <div class="settings-card">
    <h3>LLM cleanup</h3>
    <div class="provider-model-row">
      <div class="field-block field-half">
        <label for="llm-provider">Service</label>
        <select id="llm-provider" value={settings.s.llm_provider} onchange={(e) => changeLlmProvider((e.currentTarget as HTMLSelectElement).value)}>
          {#each Object.keys(LLM_MODELS) as provider}
            <option value={provider} disabled={!llmReady(provider)}>
              {providerLabel(provider)} {llmReady(provider) ? "" : "(add key first)"}
            </option>
          {/each}
        </select>
      </div>
      <div class="field-block field-half">
        <label for="llm-model">Model</label>
        <select id="llm-model" value={settings.s.llm_model} onchange={(e) => changeLlmModel((e.currentTarget as HTMLSelectElement).value)}>
          {#each llmModelsFor(settings.s.llm_provider) as m (m.id)}
            <option value={m.id}>{m.label} - {m.quality}</option>
          {/each}
        </select>
      </div>
    </div>
  </div>

  <p class="tip">
    Per-mode cleanup toggles and prompts live in <strong>Settings -> Modes</strong>.
    Hotkey bindings live in <strong>Settings -> Dictation</strong>.
  </p>

  {#if diag}
    <div class="diag-card">
      <div class="diag-head">
        <h3>Key storage status</h3>
        <button class="btn-link" onclick={refreshDiagnostic}>Refresh</button>
      </div>
      <ul class="diag-list">
        <li><span class="diag-label">Groq STT</span><span class="diag-loc {locationClass(diag.stt)}">{locationLabel(diag.stt)}</span></li>
        <li><span class="diag-label">Groq cleanup</span><span class="diag-loc {locationClass(diag.llm)}">{locationLabel(diag.llm)}</span></li>
        <li><span class="diag-label">OpenAI STT</span><span class="diag-loc {locationClass(diag.openai_stt)}">{locationLabel(diag.openai_stt)}</span></li>
        <li><span class="diag-label">OpenAI cleanup</span><span class="diag-loc {locationClass(diag.openai_llm)}">{locationLabel(diag.openai_llm)}</span></li>
        <li><span class="diag-label">Deepgram STT</span><span class="diag-loc {locationClass(diag.deepgram_stt)}">{locationLabel(diag.deepgram_stt)}</span></li>
        <li><span class="diag-label">ElevenLabs STT</span><span class="diag-loc {locationClass(diag.elevenlabs_stt)}">{locationLabel(diag.elevenlabs_stt)}</span></li>
        <li><span class="diag-label">Gemini cleanup</span><span class="diag-loc {locationClass(diag.gemini)}">{locationLabel(diag.gemini)}</span></li>
      </ul>
      {#if !diag.keyring_works && anyDiagSaved(diag)}
        <p class="diag-warn">
          The OS keyring is not accepting verified writes, so at least one key is in file fallback.
          The key still works, but keyring storage is preferred.
        </p>
      {/if}
      <p class="diag-path">File fallback location: <code>{diag.fallback_path}</code> {diag.fallback_exists ? "(exists)" : "(empty)"}</p>
    </div>
  {/if}
</section>

<style>
  .provider-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(310px, 1fr));
    gap: 16px;
    max-width: 1040px;
  }
  .provider-card {
    min-width: 0;
  }
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
    gap: 12px;
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
    white-space: nowrap;
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
