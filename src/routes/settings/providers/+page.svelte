<script lang="ts">
  import { onMount } from "svelte";
  import { api, type SecretCheck, type SecretKeyName } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import {
    LLM_MODELS,
    STT_MODELS,
    llmModelsFor,
    llmReady,
    providerLabel,
    sttModelsFor,
    sttReady,
  } from "$lib/provider-options";
  import { flash } from "$lib/settings-toast.svelte";

  type TestResult =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; models: string[] }
    | { kind: "error"; msg: string };
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

  async function refreshSecrets() {
    secretCheck = await api.checkSecrets();
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
    const options = sttModelsFor(provider);
    const stt_model = options.find((m) => m.id === settings.s.stt_model)
      ? settings.s.stt_model
      : options[0].id;
    await settings.setMany({ stt_provider: provider, stt_model } as any);
    flash(`STT provider: ${providerLabel(provider)}`);
  }

  async function changeSttModel(modelId: string) {
    await settings.set("stt_model", modelId as any);
    const label = sttModelsFor(settings.s.stt_provider).find((m) => m.id === modelId)?.label ?? modelId;
    flash(`STT: ${label}`);
  }

  async function changeLlmProvider(provider: string) {
    const options = llmModelsFor(provider);
    const llm_model = options.find((m) => m.id === settings.s.llm_model)
      ? settings.s.llm_model
      : options[0].id;
    await settings.setMany({ llm_provider: provider, llm_model } as any);
    flash(`Cleanup provider: ${providerLabel(provider)}`);
  }

  async function changeLlmModel(modelId: string) {
    await settings.set("llm_model", modelId as any);
    const label = llmModelsFor(settings.s.llm_provider).find((m) => m.id === modelId)?.label ?? modelId;
    flash(`Cleanup model: ${label}`);
  }

  onMount(() => {
    refreshSecrets();
  });
</script>

<section>
  <h2>Providers & API keys</h2>
  <p class="lede">
    Choose who transcribes your audio and who handles cleanup. Keys are saved
    locally in the OS keyring first, with encrypted fallback only when the keyring fails.
  </p>

  <div class="settings-card model-choice-card">
    <h3>Speech-to-text</h3>
    <div class="provider-model-row">
      <div class="field-block field-half">
        <label for="stt-provider">Service</label>
        <select id="stt-provider" value={settings.s.stt_provider} onchange={(e) => changeSttProvider((e.currentTarget as HTMLSelectElement).value)}>
          {#each Object.keys(STT_MODELS) as provider}
            <option value={provider} disabled={!sttReady(secretCheck, provider)}>
              {providerLabel(provider)} {sttReady(secretCheck, provider) ? "" : "(add key first)"}
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

  <div class="settings-card model-choice-card">
    <h3>LLM cleanup</h3>
    <div class="provider-model-row">
      <div class="field-block field-half">
        <label for="llm-provider">Service</label>
        <select id="llm-provider" value={settings.s.llm_provider} onchange={(e) => changeLlmProvider((e.currentTarget as HTMLSelectElement).value)}>
          {#each Object.keys(LLM_MODELS) as provider}
            <option value={provider} disabled={!llmReady(secretCheck, provider)}>
              {providerLabel(provider)} {llmReady(secretCheck, provider) ? "" : "(add key first)"}
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

  <details class="key-manager">
    <summary>
      <span>Manage API keys</span>
      <em>{secretCheck.any_stt ? "STT ready" : "Add an STT key to start"}</em>
    </summary>
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
  </details>

  <p class="tip">
    Clean Transcribe by default from the sidebar. Per-mode prompts live in <strong>Modes</strong>,
    hotkeys live in <strong>Dictation</strong>, and key storage diagnostics live in <strong>Security</strong>.
  </p>
</section>

<style>
  .provider-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(310px, 1fr));
    gap: 16px;
    max-width: 1040px;
  }
  .model-choice-card {
    max-width: 860px;
  }
  .key-manager {
    max-width: 1040px;
    margin-top: 6px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-card);
    overflow: hidden;
  }
  .key-manager > summary {
    list-style: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 18px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .key-manager > summary::-webkit-details-marker {
    display: none;
  }
  .key-manager > summary::before {
    content: "";
    width: 0;
    height: 0;
    border-top: 4px solid transparent;
    border-bottom: 4px solid transparent;
    border-left: 6px solid var(--text-secondary);
    transform-origin: 3px 4px;
    transition: transform 120ms ease;
  }
  .key-manager[open] > summary::before {
    transform: rotate(90deg);
  }
  .key-manager > summary em {
    margin-left: auto;
    font-size: 11px;
    font-style: normal;
    font-weight: 500;
    color: var(--text-secondary);
  }
  .key-manager[open] > summary {
    border-bottom: 1px solid var(--border-subtle);
  }
  .key-manager .provider-grid {
    padding: 16px;
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
</style>
