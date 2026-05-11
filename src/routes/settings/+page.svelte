<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { skinStore, setClippyWindowVisible, type Skin } from "$lib/skin-store.svelte";
  import HotkeyCapture from "$lib/HotkeyCapture.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";

  type Section = "provider" | "models" | "hotkeys" | "audio" | "look" | "retention" | "compare";
  let section = $state<Section>("provider");

  // ── Provider comparison matrix (May 2026 figures) ─────────────────────
  // Maintained manually — re-verify every ~3 months as free tiers shift.
  type ProviderRow = {
    name: string;
    site: string;
    console: string;
    sttModels: string;
    sttFree: string;
    sttPaid: string;
    llmModels: string;
    llmFree: string;
    llmPaid: string;
    notes: string;
    status: "active" | "planned" | "considered";
  };

  // Verified May 2026 — re-check every 2-3 months as free tiers shift fast.
  // Sources: Vellum, llm-stats, costgoat, pecollective, getaiperks (May 2026 round-ups).
  const PROVIDERS: ProviderRow[] = [
    {
      name: "Groq",
      site: "groq.com",
      console: "https://console.groq.com/keys",
      sttModels: "Whisper Turbo, Whisper v3, Distil-Whisper",
      sttFree: "30 RPM, no daily expiry. No card required.",
      sttPaid: "$0.04 – $0.111 / audio-hour",
      llmModels: "Llama 4 Maverick, Llama 3.3 70B, Llama 3.1 8B, Gemma 2 9B, Mixtral 8x7B",
      llmFree: "30 RPM across all models. Generous daily caps.",
      llmPaid: "$0.05 – $0.79 / M tokens",
      notes: "Fastest inference globally. Best free-tier story for STT + LLM combined. The only provider wired in.",
      status: "active",
    },
    {
      name: "Google Gemini",
      site: "ai.google.dev",
      console: "https://aistudio.google.com/app/apikey",
      sttModels: "Gemini 2.5 Flash multimodal (audio in)",
      sttFree: "1,500 req/day (Flash, free tier)",
      sttPaid: "$0.075 / M tokens (audio counted as tokens)",
      llmModels: "Gemini 3.1 Pro, 3 Pro, 2.5 Pro, 2.5 Flash, 2.0 Flash",
      llmFree: "15 RPM, 1,500 RPD on Flash. PRO MODELS NO LONGER FREE (changed Apr 2026).",
      llmPaid: "$0.10 – $5 / M tokens (Flash cheapest)",
      notes: "Most generous free tier for LLM cleanup in 2026. Flash quality is competitive with GPT-4o-mini.",
      status: "planned",
    },
    {
      name: "Anthropic (Claude)",
      site: "anthropic.com",
      console: "https://console.anthropic.com/settings/keys",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "Claude Opus 4.7, Sonnet 4.5, Haiku 4, Mythos Preview",
      llmFree: "$5 trial credits after phone verification. No ongoing free tier.",
      llmPaid: "$0.25 / $1.25 per M (Haiku) · $3 / $15 per M (Sonnet) · $15 / $75 (Opus)",
      notes: "Best drafting quality for F10. Haiku 4 = great price/perf. Startup program offers $25,000 credits.",
      status: "planned",
    },
    {
      name: "OpenAI",
      site: "openai.com",
      console: "https://platform.openai.com/api-keys",
      sttModels: "Whisper-1, GPT-4o-mini-transcribe",
      sttFree: "$5 trial credits, expires 3 months",
      sttPaid: "$0.006 / min (Whisper) · ~$0.36/hr",
      llmModels: "GPT-5.5 Pro, GPT-5.5, GPT-5.2, GPT-4.1, GPT-4.1 Nano",
      llmFree: "$5 trial credits, expires 3 months. 500 RPM unlimited RPD on 4o-mini.",
      llmPaid: "$0.10 / $0.40 per M (4.1 Nano — cheapest) · $0.15 / $0.60 (4o-mini) · much higher for GPT-5.x",
      notes: "Highest LLM quality with GPT-5.5 Pro. 4.1 Nano is the cheapest in the family — competitive with Groq.",
      status: "planned",
    },
    {
      name: "xAI Grok",
      site: "x.ai",
      console: "https://console.x.ai",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "Grok 4, Grok 3 Mini",
      llmFree: "$25 sign-up credit + $150/month via data-sharing program",
      llmPaid: "Competitive with Claude Sonnet tier",
      notes: "Most generous dollar amount in trial credits. Data-sharing program is real ongoing free use if you opt in.",
      status: "considered",
    },
    {
      name: "DeepSeek",
      site: "deepseek.com",
      console: "https://platform.deepseek.com",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "DeepSeek V3.2, V3.1, R1 (reasoning)",
      llmFree: "Trial credits on signup",
      llmPaid: "$0.14 / $0.28 per M (V3.2) — cheapest near-frontier mainstream",
      notes: "Strong quality at very low cost. R1 model is comparable to GPT-5.2 on math. Heavy users save 50%+ vs Claude.",
      status: "considered",
    },
    {
      name: "Mistral",
      site: "mistral.ai",
      console: "https://console.mistral.ai",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "Mistral Medium 3, Mistral Small, Pixtral",
      llmFree: "Limited free tier on La Plateforme",
      llmPaid: "$0.10 / $0.30 per M (Small) · $0.40 / $2 per M (Medium 3)",
      notes: "Mistral Small is tied with Gemini 2.0 Flash for cheapest paid quality LLM in 2026.",
      status: "considered",
    },
    {
      name: "Cerebras",
      site: "cerebras.ai",
      console: "https://inference.cerebras.ai",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "Llama 3.3 70B, Qwen 3 235B",
      llmFree: "1M tokens/day on signup",
      llmPaid: "$0.20 – $0.60 per M tokens",
      notes: "Fastest paid inference for big models (faster than Groq for >70B). Modest free tier.",
      status: "considered",
    },
    {
      name: "OpenRouter",
      site: "openrouter.ai",
      console: "https://openrouter.ai/settings/keys",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "Aggregator — DeepSeek R1, Llama 3.3 70B, Gemma 3 (free routes), GPT-5, Claude, etc.",
      llmFree: "Free routes available for several models (no card needed)",
      llmPaid: "Pass-through pricing + small margin",
      notes: "Single API key, many models. Best for experimentation. Pair with any STT provider.",
      status: "considered",
    },
    {
      name: "Deepgram",
      site: "deepgram.com",
      console: "https://console.deepgram.com",
      sttModels: "Nova-3 (proprietary), Whisper",
      sttFree: "$200 credit on signup (~750 hrs of Nova-3)",
      sttPaid: "$0.0044 / min · ~$0.26/hr (Nova-3, cheapest mainstream STT)",
      llmModels: "—",
      llmFree: "—",
      llmPaid: "—",
      notes: "STT-only. Nova-3 = same accuracy as Whisper at lower cost + speed. Pair with Groq/Claude LLM.",
      status: "considered",
    },
    {
      name: "AssemblyAI",
      site: "assemblyai.com",
      console: "https://www.assemblyai.com/app/account",
      sttModels: "Universal, Slam-1",
      sttFree: "416 hours / year (no card needed)",
      sttPaid: "$0.37 / hour",
      llmModels: "—",
      llmFree: "—",
      llmPaid: "—",
      notes: "STT-only. Best NO-CARD free tier for STT. Built-in diarization for meeting transcription.",
      status: "considered",
    },
    {
      name: "Sarvam",
      site: "sarvam.ai",
      console: "https://www.sarvam.ai",
      sttModels: "Saaras (IndicWhisper), Indic-Conformer",
      sttFree: "Limited free credits",
      sttPaid: "Custom pricing",
      llmModels: "Sarvam-M (Indic-tuned)",
      llmFree: "Limited free credits",
      llmPaid: "Custom pricing",
      notes: "Best Indic-language accuracy by a wide margin. Worth wiring for Hindi/Tamil/Telugu-heavy users.",
      status: "considered",
    },
    {
      name: "AWS Bedrock",
      site: "aws.amazon.com/bedrock",
      console: "https://console.aws.amazon.com/bedrock/",
      sttModels: "—",
      sttFree: "—",
      sttPaid: "—",
      llmModels: "Claude (Opus/Sonnet/Haiku), Llama, Titan, Cohere, Mistral",
      llmFree: "AWS Activate startup program: $1k-$100k credits including Bedrock",
      llmPaid: "Marked up vs direct API pricing",
      notes: "Best path to Claude credits via AWS Activate. Worth applying if you have a registered startup.",
      status: "considered",
    },
  ];

  let secretCheck = $state({ stt: false, llm: false, gemini: false });
  let groqStt = $state("");
  let groqLlm = $state("");
  let geminiKey = $state("");
  let saving = $state(false);
  let savedToast = $state<string | null>(null);

  // ── API key testing ────────────────────────────────────────────────────
  type TestResult =
    | { kind: "idle" }
    | { kind: "testing" }
    | { kind: "ok"; models: string[] }
    | { kind: "error"; msg: string };
  let testResult = $state<TestResult>({ kind: "idle" });

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

  // ── Notification sounds ────────────────────────────────────────────────
  let availableSounds = $state<string[]>([]);
  let soundsDir = $state<string>("");

  async function refreshSounds() {
    availableSounds = await api.listNotificationSounds();
    try {
      const paths = await api.appPaths();
      soundsDir = paths.sounds_dir;
    } catch {
      /* ignore */
    }
  }

  // Sync start + stop sounds: when on, picking a start sound also sets stop.
  let soundSync = $state(false);

  async function setStartSound(name: string) {
    await settings.set("start_sound", name);
    if (soundSync) {
      await settings.set("stop_sound", name);
    }
    await api.configureCues(name, soundSync ? name : settings.s.stop_sound, settings.s.cues_enabled);
    flash("Start sound updated");
  }
  async function setStopSound(name: string) {
    if (soundSync) return; // ignored when synced
    await settings.set("stop_sound", name);
    await api.configureCues(settings.s.start_sound, name, settings.s.cues_enabled);
    flash("Stop sound updated");
  }
  async function setCuesEnabled(on: boolean) {
    await settings.set("cues_enabled", on);
    await api.configureCues(settings.s.start_sound, settings.s.stop_sound, on);
  }
  function toggleSync() {
    soundSync = !soundSync;
    if (soundSync && settings.s.start_sound) {
      setStartSound(settings.s.start_sound); // mirror to stop
    }
  }

  // Preview a sound by name (or null = built-in beep — falls back to system).
  let previewAudio: HTMLAudioElement | null = null;
  async function previewSound(name: string) {
    if (previewAudio) {
      previewAudio.pause();
      previewAudio = null;
    }
    if (!name) return; // built-in default — no file to preview, skip
    const paths = await api.appPaths();
    const fileUrl = `file://${paths.sounds_dir.replace(/\\/g, "/")}/${encodeURIComponent(name)}`;
    try {
      previewAudio = new Audio(fileUrl);
      previewAudio.volume = 0.6;
      await previewAudio.play();
    } catch (e) {
      console.warn("preview failed", e);
    }
  }

  async function uploadSound() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Audio", extensions: ["wav", "mp3", "ogg"] }],
      });
      if (typeof picked === "string") {
        await api.addNotificationSound(picked);
        await refreshSounds();
        flash("Sound added");
      }
    } catch (e) {
      alert(`Upload failed: ${e}`);
    }
  }

  // ── Skin (Floater character) ───────────────────────────────────────────
  type SkinOption = { id: Skin; label: string; desc: string };
  const SKIN_OPTIONS: SkinOption[] = [
    { id: "off",         label: "Off",       desc: "Hide the floating character entirely" },
    { id: "stylized",    label: "Paperclip", desc: "Minimal stylised paperclip — clean, modern" },
    { id: "real-clippy", label: "Clippy",    desc: "The actual Microsoft Clippy with original animations" },
    { id: "chippy",      label: "Chippy",    desc: "Potato chip mascot (placeholder — improvements pending)" },
  ];
  async function pickSkin(s: Skin) {
    await skinStore.set(s);
    await setClippyWindowVisible(s !== "off");
  }

  // ── Theme ──────────────────────────────────────────────────────────────
  const THEME_OPTIONS = [
    { id: "auto",  label: "Auto",  desc: "Follow your system theme" },
    { id: "light", label: "Light", desc: "Always light" },
    { id: "dark",  label: "Dark",  desc: "Always dark" },
    { id: "retro", label: "Retro", desc: "Warm cream tones, vintage feel" },
  ] as const;

  // ── Model picker with qualitative labels ───────────────────────────────
  // STT models on Groq (May 2026). Sorted speed-first.
  type ModelOpt = { id: string; label: string; quality: string };
  const STT_MODELS: ModelOpt[] = [
    { id: "whisper-large-v3-turbo",  label: "Whisper Turbo",      quality: "Fast • Good" },
    { id: "whisper-large-v3",        label: "Whisper Large v3",   quality: "Slower • Best accuracy" },
    { id: "distil-whisper-large-v3-en", label: "Distil-Whisper",  quality: "Fastest • English only" },
  ];
  const GROQ_LLM_MODELS: ModelOpt[] = [
    { id: "llama-3.1-8b-instant",    label: "Llama 3.1 8B",    quality: "Fast • 14,400/day free" },
    { id: "llama-3.3-70b-versatile", label: "Llama 3.3 70B",   quality: "Smarter • 1,000/day free" },
    { id: "llama-4-maverick",        label: "Llama 4 Maverick", quality: "Smartest • 500/day free" },
  ];
  const GEMINI_LLM_MODELS: ModelOpt[] = [
    { id: "gemini-2.5-flash",        label: "Gemini 2.5 Flash", quality: "Fast • 1,500/day free • Best free quality" },
    { id: "gemini-2.0-flash",        label: "Gemini 2.0 Flash", quality: "Fast • free tier" },
    { id: "gemini-2.5-pro",          label: "Gemini 2.5 Pro",   quality: "Smartest • PAID ONLY since Apr 2026" },
    { id: "gemini-3-pro",            label: "Gemini 3 Pro",     quality: "Smartest • PAID ONLY" },
  ];

  function modelsForProvider(p: string): ModelOpt[] {
    return p === "gemini" ? GEMINI_LLM_MODELS : GROQ_LLM_MODELS;
  }

  // Simplified May 2026: one LLM provider/model pair for all three modes.
  // Mode (F8 / F9 / F10) only chooses which system prompt to send — the
  // model itself stays the same.
  async function changeLlmProvider(provider: string) {
    await settings.set("llm_provider", provider as any);
    const options = modelsForProvider(provider);
    if (!options.find((m) => m.id === settings.s.llm_model)) {
      await settings.set("llm_model", options[0].id as any);
    }
  }

  // Saved-key persistence: when the user toggles to a provider whose key
  // isn't saved yet, the dropdown option is disabled. Switching back later
  // to a provider whose key IS saved is always allowed — keys aren't
  // touched by provider switches, only by Delete buttons.

  // ── Lifecycle ──────────────────────────────────────────────────────────
  onMount(async () => {
    await settings.init();
    secretCheck = await api.checkSecrets();
    refreshSounds();
    skinStore.subscribe();
  });

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

  // Gemini test — uses freshly-pasted key (saved keys can't be read back from JS).
  let geminiTestResult = $state<TestResult>({ kind: "idle" });
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

  function flash(msg: string) {
    savedToast = msg;
    setTimeout(() => (savedToast = null), 1800);
  }

  let hotkeyDirty = $state(false);
  async function saveHotkeys() {
    await settings.replace({ ...settings.s });
    hotkeyDirty = false;
    flash("Saved — restart wispr-fox to apply new hotkeys");
  }

  async function setNumber<K extends keyof typeof settings.s>(key: K, value: number) {
    await settings.set(key, value as (typeof settings.s)[K]);
  }

  const SECTIONS: { id: Section; label: string; icon: string }[] = [
    { id: "provider",  label: "Provider & Keys", icon: "🔑" },
    { id: "models",    label: "Models",          icon: "✨" },
    { id: "hotkeys",   label: "Hotkeys",         icon: "⌨" },
    { id: "audio",     label: "Audio cues",      icon: "🔔" },
    { id: "look",      label: "Look & Feel",     icon: "🎨" },
    { id: "retention", label: "Retention",       icon: "🗃" },
    { id: "compare",   label: "Compare providers", icon: "📊" },
  ];
</script>

<div class="settings">
  <aside class="section-nav">
    <h1>Settings</h1>
    <nav>
      {#each SECTIONS as s (s.id)}
        <button
          class="nav-btn"
          class:active={section === s.id}
          onclick={() => (section = s.id)}
        >
          <span class="nav-icon">{s.icon}</span>
          <span>{s.label}</span>
        </button>
      {/each}
    </nav>
  </aside>

  <main class="section-body">
    {#if savedToast}
      <div class="toast">✓ {savedToast}</div>
    {/if}

    <!-- ═══ Provider & Keys ═══════════════════════════════════════════ -->
    {#if section === "provider"}
      <section>
        <h2>Provider & API keys</h2>
        <p class="lede">
          wispr-fox uses cloud providers for speech-to-text and LLM cleanup.
          Currently <strong>Groq</strong> is the supported provider — more on the roadmap.
        </p>

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
            <button class="btn-secondary" onclick={testGroqStt}>
              Test connection
            </button>
            {#if testResult.kind === "testing"}
              <span class="test-msg testing">Testing…</span>
            {:else if testResult.kind === "ok"}
              <span class="test-msg ok">✓ Key works — {testResult.models.length} models accessible</span>
            {:else if testResult.kind === "error"}
              <span class="test-msg error">✗ {testResult.msg}</span>
            {/if}
          </div>
        </div>

        <!-- ── Google Gemini ───────────────────────────────────────────── -->
        <div class="provider-card" style="margin-top: 18px;">
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
      </section>
    {/if}

    <!-- ═══ Models ══════════════════════════════════════════════════════ -->
    {#if section === "models"}
      <section>
        <h2>Models</h2>
        <p class="lede">Pick the model for each mode. Faster models hit fewer rate limits; smarter models give better output.</p>

        <h3>Speech-to-text</h3>
        <p class="lede">Which service transcribes your audio. Currently Groq Whisper only — Gemini multimodal STT is on the roadmap.</p>
        <div class="provider-model-row">
          <div class="field-block field-half">
            <label>Service</label>
            <select value={settings.s.stt_provider} onchange={(e) => settings.set("stt_provider", (e.currentTarget as HTMLSelectElement).value)}>
              <option value="groq">Groq Whisper</option>
            </select>
          </div>
          <div class="field-block field-half">
            <label>Model</label>
            <select
              value={settings.s.stt_model}
              onchange={(e) => settings.set("stt_model", (e.currentTarget as HTMLSelectElement).value)}
            >
              {#each STT_MODELS as m (m.id)}
                <option value={m.id}>{m.label} — {m.quality}</option>
              {/each}
            </select>
          </div>
        </div>

        <h3 style="margin-top: 28px;">LLM cleanup</h3>
        <p class="lede">
          Used by F9 / F10 (and F8 if you've enabled cleanup for it). One choice — the same model handles all three modes,
          only the prompt changes per mode. Your saved API keys stick around when you switch providers, so you can flip
          back and forth without re-entering them.
        </p>
        <div class="provider-model-row">
          <div class="field-block field-half">
            <label>Service</label>
            <select value={settings.s.llm_provider} onchange={(e) => changeLlmProvider((e.currentTarget as HTMLSelectElement).value)}>
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
              onchange={(e) => settings.set("llm_model", (e.currentTarget as HTMLSelectElement).value)}
            >
              {#each modelsForProvider(settings.s.llm_provider) as m (m.id)}
                <option value={m.id}>{m.label} — {m.quality}</option>
              {/each}
            </select>
          </div>
        </div>

        <h3 style="margin-top: 28px;">LLM cleanup per mode</h3>
        <p class="lede">When enabled, the raw Whisper transcript gets passed through the chosen LLM for cleanup or transformation. Light mode is OFF by default — Whisper is accurate enough that an extra LLM pass mostly adds latency.</p>
        <div class="field-block">
          <label class="check-row">
            <input
              type="checkbox"
              checked={settings.s.auto_clean_in_light}
              onchange={(e) => settings.set("auto_clean_in_light", (e.currentTarget as HTMLInputElement).checked)}
            />
            <span><strong>F8 Light</strong> — punctuation + capitalisation cleanup. Default: <em>off</em>.</span>
          </label>
          <label class="check-row" style="margin-top: 6px;">
            <input
              type="checkbox"
              checked={settings.s.auto_clean_in_advanced}
              onchange={(e) => settings.set("auto_clean_in_advanced", (e.currentTarget as HTMLInputElement).checked)}
            />
            <span><strong>F9 Advanced</strong> — instruction-aware transformation. Default: <em>on</em>. (Disabling makes F9 behave like F8.)</span>
          </label>
          <label class="check-row" style="margin-top: 6px;">
            <input
              type="checkbox"
              checked={settings.s.auto_clean_in_drafting}
              onchange={(e) => settings.set("auto_clean_in_drafting", (e.currentTarget as HTMLInputElement).checked)}
            />
            <span><strong>F10 Drafting</strong> — full draft generation from your brief. Default: <em>on</em>.</span>
          </label>
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
      </section>
    {/if}

    <!-- ═══ Hotkeys ═════════════════════════════════════════════════════ -->
    {#if section === "hotkeys"}
      <section>
        <h2>Hotkeys</h2>
        <p class="lede">
          Three modes, each with TWO hotkeys: a <strong>main</strong> hotkey (push-to-talk by default —
          hold to record) and a <strong>sticky-invoke</strong> hotkey (always toggles —
          press once to start, press again to stop). Pressing the sticky combo gives you
          a one-shot sticky session without permanently changing the mode behaviour.
        </p>
        <p class="lede" style="margin-top: -10px;">
          Optionally check <strong>"Default to sticky"</strong> to make the main hotkey also behave as a toggle.
        </p>

        <div class="hotkey-block">
          <div class="hotkey-head">
            <div>
              <div class="hk-label">Light <span class="hk-tag">most used</span></div>
              <div class="hk-desc">Cleanup-only — fixes punctuation + capitalisation, never rephrases.</div>
            </div>
          </div>
          <div class="hk-pair">
            <div class="hk-pair-col">
              <div class="hk-pair-label">Main (push-to-talk)</div>
              <HotkeyCapture label="" bind:value={settings.s.light_hotkey} />
            </div>
            <div class="hk-pair-col">
              <div class="hk-pair-label">Sticky-invoke (toggle)</div>
              <HotkeyCapture label="" bind:value={settings.s.light_sticky_hotkey} />
            </div>
          </div>
          <label class="check-row small">
            <input type="checkbox" checked={settings.s.sticky_light}
                   onchange={(e) => settings.set("sticky_light", (e.currentTarget as HTMLInputElement).checked)} />
            <span>Default to sticky — make the MAIN hotkey behave as toggle too</span>
          </label>
        </div>

        <div class="hotkey-block">
          <div class="hotkey-head">
            <div>
              <div class="hk-label">Drafting <span class="hk-tag">F10 default · second most used</span></div>
              <div class="hk-desc">Drafts polished output from your brief. Best for emails, slack messages, docs.</div>
            </div>
          </div>
          <div class="hk-pair">
            <div class="hk-pair-col">
              <div class="hk-pair-label">Main (push-to-talk)</div>
              <HotkeyCapture label="" bind:value={settings.s.drafting_hotkey} />
            </div>
            <div class="hk-pair-col">
              <div class="hk-pair-label">Sticky-invoke (toggle)</div>
              <HotkeyCapture label="" bind:value={settings.s.drafting_sticky_hotkey} />
            </div>
          </div>
          <label class="check-row small">
            <input type="checkbox" checked={settings.s.sticky_drafting}
                   onchange={(e) => settings.set("sticky_drafting", (e.currentTarget as HTMLInputElement).checked)} />
            <span>Default to sticky</span>
          </label>
        </div>

        <div class="hotkey-block">
          <div class="hotkey-head">
            <div>
              <div class="hk-label">Advanced <span class="hk-tag">F9 default</span></div>
              <div class="hk-desc">Instruction-aware — "make this an email", "shorter", "in bullets".</div>
            </div>
          </div>
          <div class="hk-pair">
            <div class="hk-pair-col">
              <div class="hk-pair-label">Main (push-to-talk)</div>
              <HotkeyCapture label="" bind:value={settings.s.advanced_hotkey} />
            </div>
            <div class="hk-pair-col">
              <div class="hk-pair-label">Sticky-invoke (toggle)</div>
              <HotkeyCapture label="" bind:value={settings.s.advanced_sticky_hotkey} />
            </div>
          </div>
          <label class="check-row small">
            <input type="checkbox" checked={settings.s.sticky_advanced}
                   onchange={(e) => settings.set("sticky_advanced", (e.currentTarget as HTMLInputElement).checked)} />
            <span>Default to sticky</span>
          </label>
        </div>

        <button class="btn-primary" onclick={saveHotkeys}>Save hotkeys</button>
        <p class="hint">⚠ Hotkey changes take effect after restarting wispr-fox.</p>
      </section>
    {/if}

    <!-- ═══ Audio cues ══════════════════════════════════════════════════ -->
    {#if section === "audio"}
      <section>
        <h2>Audio cues</h2>
        <p class="lede">Short sounds that play when recording starts and stops. Click any tile to preview it.</p>

        <label class="check-row">
          <input type="checkbox" checked={settings.s.cues_enabled} onchange={(e) => setCuesEnabled((e.currentTarget as HTMLInputElement).checked)} />
          <span>Play audio cues on record start / stop</span>
        </label>

        {#if settings.s.cues_enabled}
          <label class="check-row" style="margin-top: 12px;">
            <input type="checkbox" checked={soundSync} onchange={toggleSync} />
            <span>Sync start + stop sounds — use the same file for both</span>
          </label>

          <div class="sound-section">
            <div class="sound-section-head">
              <span class="sound-section-title">Start sound</span>
              <button class="btn-secondary small" onclick={uploadSound}>+ Upload file</button>
            </div>
            <div class="sound-tiles">
              <button
                class="sound-tile"
                class:active={settings.s.start_sound === ""}
                onclick={() => setStartSound("")}
              >
                <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
                  <path d="M 5 9 L 5 15 L 9 15 L 14 19 L 14 5 L 9 9 Z M 18 8 L 18 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>
                </svg>
                <span class="sound-tile-label">Built-in tone</span>
              </button>
              {#each availableSounds as f (f)}
                <button
                  class="sound-tile"
                  class:active={settings.s.start_sound === f}
                  onclick={() => { setStartSound(f); previewSound(f); }}
                  title="Click to select + preview"
                >
                  <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
                    <path d="M 4 12 L 7 12 M 9 8 L 9 16 M 12 6 L 12 18 M 15 9 L 15 15 M 18 11 L 18 13" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                  </svg>
                  <span class="sound-tile-label">{f}</span>
                </button>
              {/each}
            </div>
          </div>

          <div class="sound-section" class:disabled={soundSync}>
            <div class="sound-section-head">
              <span class="sound-section-title">
                Stop sound
                {#if soundSync}<span class="sound-locked">— synced with start</span>{/if}
              </span>
            </div>
            <div class="sound-tiles">
              <button
                class="sound-tile"
                class:active={(soundSync ? settings.s.start_sound : settings.s.stop_sound) === ""}
                onclick={() => setStopSound("")}
                disabled={soundSync}
              >
                <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
                  <path d="M 5 9 L 5 15 L 9 15 L 14 19 L 14 5 L 9 9 Z M 18 8 L 18 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>
                </svg>
                <span class="sound-tile-label">Built-in tone</span>
              </button>
              {#each availableSounds as f (f)}
                <button
                  class="sound-tile"
                  class:active={(soundSync ? settings.s.start_sound : settings.s.stop_sound) === f}
                  onclick={() => { setStopSound(f); previewSound(f); }}
                  disabled={soundSync}
                >
                  <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
                    <path d="M 4 12 L 7 12 M 9 8 L 9 16 M 12 6 L 12 18 M 15 9 L 15 15 M 18 11 L 18 13" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                  </svg>
                  <span class="sound-tile-label">{f}</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}

        <div class="folder-hint">
          <div>Sound files live at:</div>
          <code>{soundsDir || "%APPDATA%\\com.wispr-fox.app\\sounds\\"}</code>
          <div class="folder-hint-actions">
            <button class="btn-secondary small" onclick={refreshSounds}>Refresh</button>
            <button class="btn-secondary small" onclick={async () => {
              const { openPath } = await import("@tauri-apps/plugin-opener");
              await openPath(soundsDir);
            }}>Open folder</button>
          </div>
        </div>
      </section>
    {/if}

    <!-- ═══ Look & Feel ═════════════════════════════════════════════════ -->
    {#if section === "look"}
      <section>
        <h2>Look & Feel</h2>

        <h3>Floater character</h3>
        <p class="lede">The floating animated character that reacts to your dictation. Off = hidden window.</p>
        <div class="skin-tiles">
          {#each SKIN_OPTIONS as opt (opt.id)}
            <button
              class="skin-tile"
              class:active={skinStore.current === opt.id}
              onclick={() => pickSkin(opt.id)}
              title={opt.desc}
            >
              <div class="skin-tile-preview">
                <!-- Uses the shared SkinIcon component — real Microsoft Clippy
                     bitmap for real-clippy, SVG for others. Consistency with
                     the sidebar picker. -->
                <SkinIcon skin={opt.id} size={60} />
              </div>
              <div class="skin-tile-label">{opt.label}</div>
              {#if skinStore.current === opt.id}
                <div class="skin-tile-check">✓</div>
              {/if}
            </button>
          {/each}
        </div>

        <h3>Theme</h3>
        <p class="lede">App-wide colour scheme. (Retro warm theme is in progress — placeholder for now.)</p>
        <div class="radio-grid">
          {#each THEME_OPTIONS as opt (opt.id)}
            <button
              class="radio-card"
              class:active={settings.s.theme === opt.id}
              onclick={() => settings.set("theme", opt.id)}
            >
              <div class="radio-card-head">
                <span class="radio-dot">{settings.s.theme === opt.id ? "●" : "○"}</span>
                <span class="radio-label">{opt.label}</span>
              </div>
              <div class="radio-desc">{opt.desc}</div>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    <!-- ═══ Compare providers ═══════════════════════════════════════════ -->
    {#if section === "compare"}
      <section>
        <h2>Compare providers</h2>
        <p class="lede">
          Cloud providers offering speech-to-text and/or LLM cleanup. Figures verified May 2026 —
          tiers shift; double-check on each provider's site before committing.
        </p>

        <div class="legend">
          <span class="badge active">Active</span> wired into the app
          <span class="badge planned">Planned</span> on the roadmap
          <span class="badge considered">Considered</span> not committed yet
        </div>

        <div class="compare-grid">
          {#each PROVIDERS as p (p.name)}
            <article class="provider-row" class:active={p.status === "active"}>
              <div class="pr-head">
                <div class="pr-title">
                  <span class="pr-name">{p.name}</span>
                  <span class="badge {p.status}">{p.status}</span>
                </div>
                <a class="link-out small" href={p.console} target="_blank" rel="noopener">
                  Get key →
                </a>
              </div>

              <div class="pr-grid">
                <div class="pr-col">
                  <div class="pr-col-head">Speech-to-text</div>
                  <div class="pr-line"><span class="pr-key">Models</span><span>{p.sttModels}</span></div>
                  <div class="pr-line"><span class="pr-key">Free</span><span>{p.sttFree}</span></div>
                  <div class="pr-line"><span class="pr-key">Paid</span><span>{p.sttPaid}</span></div>
                </div>
                <div class="pr-col">
                  <div class="pr-col-head">LLM cleanup</div>
                  <div class="pr-line"><span class="pr-key">Models</span><span>{p.llmModels}</span></div>
                  <div class="pr-line"><span class="pr-key">Free</span><span>{p.llmFree}</span></div>
                  <div class="pr-line"><span class="pr-key">Paid</span><span>{p.llmPaid}</span></div>
                </div>
              </div>

              <div class="pr-notes">{p.notes}</div>
            </article>
          {/each}
        </div>

        <div class="help-box footnote">
          <strong>Pricing notation:</strong> "$X / $Y per M tokens" means $X per million input tokens, $Y per million output tokens.
          1 M tokens ≈ 750,000 English words. A typical dictation cleanup uses 100–500 tokens combined.
        </div>

        <h3 style="margin-top: 28px;">Stack-able free credit programs</h3>
        <p class="lede">Beyond per-provider free tiers — these are one-time / ongoing credit grants worth applying for.</p>
        <div class="help-box">
          <ul>
            <li><strong>AWS Activate</strong> — $1,000-$100,000 in credits including Bedrock (Claude). Requires registered startup. <a href="https://aws.amazon.com/activate/" target="_blank" rel="noopener" class="link-out">apply →</a></li>
            <li><strong>Google for Startups Cloud</strong> — up to $350,000 in GCP credits + $10,000 in Anthropic/Claude credits at Scale tier. <a href="https://cloud.google.com/startup" target="_blank" rel="noopener" class="link-out">apply →</a></li>
            <li><strong>Anthropic Startup Program</strong> — $25,000 in Claude credits, 12 months. <a href="https://www.anthropic.com/contact-sales/for-startups" target="_blank" rel="noopener" class="link-out">apply →</a></li>
            <li><strong>OpenAI Startup Program</strong> — up to $100,000 in OpenAI credits. <a href="https://openai.com/forum/" target="_blank" rel="noopener" class="link-out">apply →</a></li>
            <li><strong>Anthropic AI for Science</strong> — up to $20,000 for research, 6 months. Academic/scientific use.</li>
            <li><strong>xAI Grok data-sharing</strong> — $150/month ongoing if you opt into data sharing.</li>
            <li><strong>Microsoft for Startups Founders Hub</strong> — Azure + OpenAI credits.</li>
            <li><strong>Together AI Startup Program</strong> — credits for open-model inference.</li>
          </ul>
          <p style="margin-top: 8px; font-size: 11px;">
            Stack-able estimate: $1,000 trivially (just sign-ups), $50k+ if you qualify as a startup, $200k+ for funded startups stacking AWS + Google + Anthropic + Azure.
          </p>
        </div>
      </section>
    {/if}

    <!-- ═══ Retention ═══════════════════════════════════════════════════ -->
    {#if section === "retention"}
      <section>
        <h2>Retention</h2>
        <p class="lede">How long recordings (audio + transcript) are kept before automatic cleanup.</p>

        <div class="field-block">
          <label>Retention: {settings.s.retention_days} day{settings.s.retention_days === 1 ? "" : "s"}</label>
          <input
            type="range"
            min="1"
            max="90"
            value={settings.s.retention_days}
            oninput={(e) => setNumber("retention_days", Number((e.currentTarget as HTMLInputElement).value))}
          />
          <p class="hint">Recordings older than this are auto-deleted hourly. Affects both audio files and text.</p>
        </div>

        <div class="field-block">
          <label>Storage cap (MB)</label>
          <input
            type="number"
            min="50"
            step="50"
            value={settings.s.retention_max_mb}
            onchange={(e) => setNumber("retention_max_mb", Number((e.currentTarget as HTMLInputElement).value))}
          />
          <p class="hint">When the audio folder exceeds this, the oldest files are removed first.</p>
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 200px 1fr;
    height: 100vh;
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .section-nav {
    border-right: 1px solid var(--border);
    padding: 18px 12px;
    overflow-y: auto;
    background: var(--bg-sidebar);
  }

  .section-nav h1 {
    font-size: 18px;
    font-weight: 600;
    margin: 0 0 14px 8px;
    color: var(--text-primary);
  }

  .section-nav nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    background: transparent;
    border: none;
    border-radius: 7px;
    cursor: pointer;
    text-align: left;
    font-size: 13px;
    color: var(--text-primary);
    transition: background 100ms ease;
  }

  .nav-btn:hover {
    background: var(--bg-subtle);
  }

  .nav-btn.active {
    background: var(--accent-fade);
    color: var(--accent);
    font-weight: 500;
  }

  .nav-icon {
    width: 18px;
    text-align: center;
    font-size: 13px;
  }

  .section-body {
    overflow-y: auto;
    padding: 28px 36px;
    position: relative;
  }

  section h2 {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 6px;
  }

  section h3 {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 24px 0 8px;
  }

  .lede {
    font-size: 13px;
    color: var(--text-secondary);
    margin: 0 0 18px;
    max-width: 620px;
    line-height: 1.5;
  }

  .toast {
    position: absolute;
    top: 16px;
    right: 24px;
    background: rgba(52, 199, 89, 0.14);
    color: #1c7c3a;
    padding: 6px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    z-index: 10;
  }

  .provider-card {
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-card);
    padding: 18px;
    max-width: 720px;
  }

  .provider-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  .provider-name {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .provider-meta {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .link-out {
    font-size: 12px;
    color: #0a84ff;
    text-decoration: none;
  }

  .link-out:hover {
    text-decoration: underline;
  }

  .help-box {
    background: rgba(0, 0, 0, 0.025);
    border-radius: 8px;
    padding: 10px 14px;
    margin-bottom: 16px;
    font-size: 12px;
    color: var(--text-primary);
  }

  .help-box ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }

  .help-box li {
    margin: 3px 0;
    color: var(--text-secondary);
  }

  .help-box li code {
    background: rgba(0, 0, 0, 0.06);
    padding: 1px 5px;
    border-radius: 3px;
    color: var(--text-primary);
  }

  .key-row {
    margin-bottom: 14px;
  }

  .key-row label {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 4px;
  }

  .input-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .input-row input {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 12px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
  }

  .test-block {
    margin-top: 10px;
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .test-msg {
    font-size: 12px;
  }

  .test-msg.testing { color: var(--text-secondary); }
  .test-msg.ok      { color: #1c7c3a; }
  .test-msg.error   { color: #b3261e; word-break: break-word; }

  .btn-primary {
    background: #0a84ff;
    color: #fff;
    border: none;
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
    transition: background 100ms ease;
  }
  .btn-primary:hover:not(:disabled) {
    background: #0070dd;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: var(--bg-card);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
  }
  .btn-secondary.small {
    padding: 3px 10px;
    font-size: 11px;
  }
  .btn-secondary:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  .btn-danger {
    background: transparent;
    color: #ff453a;
    border: 1px solid rgba(255, 69, 58, 0.3);
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 12px;
    cursor: pointer;
  }
  .btn-danger:hover {
    background: rgba(255, 69, 58, 0.08);
  }

  .field-block {
    margin-bottom: 16px;
    max-width: 620px;
  }

  .field-block label {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 5px;
  }

  .hint-inline {
    font-weight: 400;
    color: var(--text-secondary);
  }

  .field-block input, .field-block select {
    width: 100%;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 12px;
    background: var(--bg-card);
    font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
  }

  .field-block input[type="range"] {
    padding: 0;
  }

  .provider-model-row {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 10px;
    max-width: 620px;
  }

  .field-half {
    margin-bottom: 0;
    max-width: none;
  }

  .hint {
    font-size: 11px;
    color: var(--text-secondary);
    margin: 4px 0 0;
  }

  .hint code {
    background: rgba(0, 0, 0, 0.06);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-primary);
  }

  .check-row.small {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 6px;
  }

  .hotkey-block {
    border: 1px solid var(--border);
    background: var(--bg-card);
    border-radius: 10px;
    padding: 14px;
    margin-bottom: 12px;
    max-width: 620px;
  }

  .hotkey-head {
    margin-bottom: 8px;
  }

  .hk-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .hk-tag {
    font-size: 10px;
    font-weight: 400;
    color: var(--text-secondary);
    padding: 1px 6px;
    background: rgba(0, 0, 0, 0.05);
    border-radius: 4px;
    margin-left: 4px;
  }

  .hk-desc {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .hk-pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-bottom: 6px;
  }

  .hk-pair-col {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .hk-pair-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .folder-hint {
    margin-top: 24px;
    padding: 12px 14px;
    background: var(--bg-subtle);
    border-radius: 8px;
    font-size: 11px;
    color: var(--text-secondary);
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 620px;
  }

  .folder-hint code {
    color: var(--text-primary);
    background: var(--bg-card);
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid var(--border-subtle);
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 11px;
    word-break: break-all;
  }

  .folder-hint-actions {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }

  /* Sound picker */
  .sound-section {
    margin-top: 20px;
    max-width: 620px;
  }
  .sound-section.disabled {
    opacity: 0.5;
  }
  .sound-section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .sound-section-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .sound-locked {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-secondary);
    margin-left: 4px;
  }

  .sound-tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
  }
  .sound-tile {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 9px;
    cursor: pointer;
    color: var(--text-primary);
    text-align: left;
    transition: all 120ms ease;
  }
  .sound-tile:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--bg-subtle);
  }
  .sound-tile.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    color: var(--accent);
    font-weight: 500;
  }
  .sound-tile:disabled {
    cursor: not-allowed;
  }
  .sound-tile-label {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  /* Skin picker tiles */
  .skin-tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 10px;
    max-width: 720px;
  }
  .skin-tile {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 16px 12px 14px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    cursor: pointer;
    color: var(--text-primary);
    transition: all 150ms ease;
  }
  .skin-tile:hover {
    border-color: var(--accent);
    transform: translateY(-2px);
    box-shadow: 0 4px 12px var(--accent-fade);
  }
  .skin-tile.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .skin-tile-preview {
    width: 70px;
    height: 70px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-primary);
  }
  .skin-tile-label {
    font-size: 12px;
    font-weight: 500;
  }
  .skin-tile-check {
    position: absolute;
    top: 6px;
    right: 8px;
    color: var(--accent);
    font-size: 14px;
    font-weight: 600;
  }

  .radio-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    max-width: 620px;
    margin-bottom: 8px;
  }

  .radio-card {
    text-align: left;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .radio-card:hover {
    border-color: rgba(10, 132, 255, 0.4);
  }

  .radio-card.active {
    border-color: #0a84ff;
    background: rgba(10, 132, 255, 0.05);
  }

  .radio-card-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .radio-dot {
    color: #0a84ff;
    font-size: 13px;
  }

  .radio-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .radio-desc {
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  /* Compare providers section */
  .legend {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: 11px;
    color: var(--text-secondary);
    margin-bottom: 14px;
  }

  .badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 9999px;
    font-size: 10px;
    font-weight: 500;
    text-transform: capitalize;
  }

  .badge.active {
    background: rgba(52, 199, 89, 0.16);
    color: #1c7c3a;
  }

  .badge.planned {
    background: rgba(10, 132, 255, 0.14);
    color: #0a84ff;
  }

  .badge.considered {
    background: rgba(0, 0, 0, 0.06);
    color: var(--text-secondary);
  }

  .compare-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 920px;
  }

  .provider-row {
    border: 1px solid var(--border);
    background: var(--bg-card);
    border-radius: 10px;
    padding: 14px 16px;
  }

  .provider-row.active {
    border-color: rgba(52, 199, 89, 0.4);
    background: rgba(52, 199, 89, 0.03);
  }

  .pr-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .pr-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pr-name {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .link-out.small {
    font-size: 11px;
  }

  .pr-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-bottom: 8px;
  }

  .pr-col {
    background: rgba(0, 0, 0, 0.025);
    border-radius: 7px;
    padding: 8px 10px;
  }

  .pr-col-head {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 6px;
  }

  .pr-line {
    display: grid;
    grid-template-columns: 50px 1fr;
    gap: 6px;
    font-size: 11px;
    color: var(--text-primary);
    margin: 2px 0;
    line-height: 1.4;
  }

  .pr-key {
    color: var(--text-secondary);
    font-weight: 500;
  }

  .pr-notes {
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin-top: 4px;
  }

  .help-box.footnote {
    margin-top: 18px;
    max-width: 920px;
  }
</style>
