import type { SecretCheck } from "./api";
import { settings } from "./settings-store.svelte";

export type ProviderModel = {
  id: string;
  label: string;
  quality: string;
};

export type ProviderOption = {
  id: string;
  label: string;
  summary: string;
};

export const DEEPGRAM_FREE_CREDIT_USD = 200;
export const DEEPGRAM_NOVA3_MULTILINGUAL_USD_PER_MIN = 0.0092;

export const STT_PROVIDERS: ProviderOption[] = [
  { id: "groq", label: "Groq", summary: "Whisper STT" },
  { id: "openai", label: "OpenAI", summary: "GPT transcription" },
  { id: "deepgram", label: "Deepgram", summary: "Nova transcription" },
  { id: "elevenlabs", label: "ElevenLabs", summary: "Scribe transcription" },
];

export const LLM_PROVIDERS: ProviderOption[] = [
  { id: "groq", label: "Groq", summary: "GPT-OSS / Qwen cleanup" },
  { id: "openai", label: "OpenAI", summary: "GPT cleanup" },
  { id: "gemini", label: "Gemini", summary: "Google cleanup" },
];

export const STT_MODELS: Record<string, ProviderModel[]> = {
  groq: [
    { id: "whisper-large-v3-turbo", label: "Whisper Turbo", quality: "Fast, strong default" },
    { id: "whisper-large-v3", label: "Whisper Large v3", quality: "Slower, highest Whisper accuracy" },
    // "distil-whisper-large-v3-en" removed 2026-07 — Groq retired the model
    // upstream (live-verified, see wispr-fox-android/HANDOVER.md "Desktop
    // parity ports"). Saved selections coerce via settings-store.svelte.ts.
  ],
  openai: [
    { id: "gpt-transcribe", label: "GPT Transcribe", quality: "Recommended for ordinary recordings" },
    { id: "gpt-4o-transcribe-diarize", label: "GPT-4o Diarize", quality: "Speaker labels for meetings" },
    { id: "gpt-4o-transcribe", label: "GPT-4o Transcribe", quality: "Best OpenAI STT quality" },
    { id: "gpt-4o-mini-transcribe", label: "GPT-4o mini Transcribe", quality: "Lower cost, fast" },
    { id: "whisper-1", label: "Whisper API", quality: "Legacy compatible fallback" },
  ],
  deepgram: [
    { id: "nova-3", label: "Nova-3", quality: "Recommended Deepgram model" },
    { id: "nova-2", label: "Nova-2", quality: "Stable fallback" },
  ],
  elevenlabs: [
    { id: "scribe_v2", label: "Scribe v2", quality: "Recommended ElevenLabs STT" },
    // "scribe_v1" removed 2026-07 — ElevenLabs retired the model upstream
    // (live-verified, see wispr-fox-android/HANDOVER.md "Desktop parity
    // ports"). Saved selections coerce via settings-store.svelte.ts.
  ],
};

export const LLM_MODELS: Record<string, ProviderModel[]> = {
  groq: [
    { id: "openai/gpt-oss-20b", label: "GPT-OSS 20B", quality: "Fast cleanup and titles — free tier" },
    { id: "openai/gpt-oss-120b", label: "GPT-OSS 120B", quality: "Stronger drafts and meeting notes — free tier" },
    { id: "qwen/qwen3.6-27b", label: "Qwen 3.6 27B", quality: "Multilingual alternative (preview, ~5x the price)" },
    { id: "qwen/qwen3.8-27b", label: "Qwen 3.8 27B", quality: "Newest Qwen (preview, ~7x the price)" },
    // Catalog check 2026-09-14 against console.groq.com/docs/models:
    // production = gpt-oss-20b/120b + Whisper; Llama 3.1/3.3 are now
    // Enterprise-only ("Contact Sales") so they are not offered here — saved
    // Llama selections coerce to GPT-OSS in settings-store.svelte.ts and
    // flow.rs. Qwen 3.6/3.8 are PREVIEW (may be pulled at short notice) and
    // reasoning models — groq.rs sends reasoning_format=hidden for them.
    // "llama-4-maverick" removed 2026-07 — never a valid Groq id.
  ],
  openai: [
    { id: "gpt-5.6-terra", label: "GPT-5.6 Terra", quality: "Fast OpenAI cleanup default" },
    { id: "gpt-5.6-sol", label: "GPT-5.6 Sol", quality: "Highest quality, slower/costlier" },
    { id: "gpt-5.6-luna", label: "GPT-5.6 Luna", quality: "Cheapest and fastest" },
  ],
  gemini: [
    { id: "gemini-3.8-flash", label: "Gemini 3.8 Flash", quality: "Latest stable Flash" },
    { id: "gemini-3.7-flash", label: "Gemini 3.7 Flash", quality: "Previous stable Flash" },
    { id: "gemini-3.6-flash", label: "Gemini 3.6 Flash", quality: "Stable balance model" },
    { id: "gemini-3.5-flash", label: "Gemini 3.5 Flash", quality: "Stable balance fallback" },
    { id: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash-Lite", quality: "Fastest current stable model" },
    { id: "gemini-3.1-flash-lite", label: "Gemini 3.1 Flash-Lite", quality: "Stable low-cost fallback" },
    { id: "gemini-2.5-flash", label: "Gemini 2.5 Flash", quality: "Fast free-tier option" },
    { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite", quality: "Lowest latency / quota friendly" },
    { id: "gemini-3-flash-preview", label: "Gemini 3 Flash Preview", quality: "Preview, if listed for your key" },
    { id: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro Preview", quality: "Preview Pro, billing likely required" },
    { id: "gemini-2.5-pro", label: "Gemini 2.5 Pro", quality: "Paid tier" },
  ],
};

export function providerLabel(provider: string): string {
  return [...STT_PROVIDERS, ...LLM_PROVIDERS].find((p) => p.id === provider)?.label ?? provider;
}

export function sttModelsFor(provider: string): ProviderModel[] {
  return STT_MODELS[provider] ?? STT_MODELS.groq;
}

export function llmModelsFor(provider: string): ProviderModel[] {
  return LLM_MODELS[provider] ?? LLM_MODELS.groq;
}

export function sttReady(secrets: SecretCheck | null, provider: string): boolean {
  if (!secrets) return true;
  return provider === "groq"
    ? secrets.stt
    : provider === "openai"
      ? secrets.openai_stt || secrets.openai_llm
      : provider === "deepgram"
        ? secrets.deepgram_stt
        : provider === "elevenlabs"
          ? secrets.elevenlabs_stt
          : false;
}

export function llmReady(secrets: SecretCheck | null, provider: string): boolean {
  if (!secrets) return true;
  return provider === "groq"
    ? secrets.llm || secrets.stt
    : provider === "openai"
      ? secrets.openai_llm || secrets.openai_stt
      : provider === "gemini"
        ? Boolean(secrets.gemini)
        : false;
}

// -- Shared provider/model switching --------------------------------------
// The sidebar (`+layout.svelte`) and Settings → Providers both drive the same
// four state changes. These helpers own the settings write + model-fallback
// logic so the two call sites don't drift; each site layers its own UI feedback
// (sidebar refreshes usage bars, the providers page adds flash() toasts) on top.

/** Switch STT provider, keeping the current model if the new provider offers
 *  it, else falling back to that provider's first model. */
export async function applySttProvider(provider: string): Promise<void> {
  const options = sttModelsFor(provider);
  const stt_model = options.some((m) => m.id === settings.s.stt_model)
    ? settings.s.stt_model
    : options[0].id;
  await settings.setMany({ stt_provider: provider, stt_model } as any);
}

export async function applySttModel(modelId: string): Promise<void> {
  await settings.set("stt_model", modelId as any);
}

/** Switch LLM provider, keeping the current model if valid, else its first. */
export async function applyLlmProvider(provider: string): Promise<void> {
  const options = llmModelsFor(provider);
  const llm_model = options.some((m) => m.id === settings.s.llm_model)
    ? settings.s.llm_model
    : options[0].id;
  await settings.setMany({ llm_provider: provider, llm_model } as any);
}

export async function applyLlmModel(modelId: string): Promise<void> {
  await settings.set("llm_model", modelId as any);
}

/** Switch the auto-title provider, keeping the current model if valid, else
 *  its first. Same shape as applyLlmProvider — titles get their own pick so a
 *  five-word name doesn't have to run on the cleanup model. */
export async function applyTitleProvider(provider: string): Promise<void> {
  const options = llmModelsFor(provider);
  const title_model = options.some((m) => m.id === settings.s.title_model)
    ? settings.s.title_model
    : options[0].id;
  await settings.setMany({ title_provider: provider, title_model } as any);
}

export async function applyTitleModel(modelId: string): Promise<void> {
  await settings.set("title_model", modelId as any);
}

export function shortModel(name: string | undefined): string {
  if (!name) return "-";
  if (name.startsWith("whisper-large-v3-turbo")) return "Whisper Turbo";
  if (name.startsWith("whisper-large-v3")) return "Whisper v3";
  if (name.startsWith("distil-whisper")) return "Distil Whisper";
  if (name === "gpt-transcribe") return "GPT Transcribe";
  if (name.startsWith("gpt-4o-transcribe-diarize")) return "4o Diarize";
  if (name.startsWith("gpt-4o-mini-transcribe")) return "4o mini STT";
  if (name.startsWith("gpt-4o-transcribe")) return "4o STT";
  if (name.startsWith("nova-3")) return "Nova-3";
  if (name.startsWith("nova-2")) return "Nova-2";
  if (name.startsWith("scribe_v2")) return "Scribe v2";
  if (name.startsWith("scribe_v1")) return "Scribe v1";
  if (name === "openai/gpt-oss-20b") return "GPT-OSS 20B";
  if (name === "openai/gpt-oss-120b") return "GPT-OSS 120B";
  if (name === "qwen/qwen3.6-27b") return "Qwen 3.6 27B";
  if (name === "qwen/qwen3.8-27b") return "Qwen 3.8 27B";
  if (name.startsWith("llama-3.3-70b")) return "Llama 70B";
  if (name.startsWith("llama-3.1-8b")) return "Llama 8B";
  if (name.startsWith("gpt-5.6-terra")) return "GPT-5.6 Terra";
  if (name.startsWith("gpt-5.6-sol")) return "GPT-5.6 Sol";
  if (name.startsWith("gpt-5.6-luna")) return "GPT-5.6 Luna";
  if (name.startsWith("gemini-3.8-flash")) return "Gemini 3.8 Flash";
  if (name.startsWith("gemini-3.7-flash")) return "Gemini 3.7 Flash";
  if (name.startsWith("gemini-3.6-flash")) return "Gemini 3.6 Flash";
  if (name.startsWith("gemini-3.5-flash-lite")) return "Gemini 3.5 Flash-Lite";
  if (name.startsWith("gemini-3.5-flash")) return "Gemini 3.5 Flash";
  if (name.startsWith("gemini-2.5-flash-lite")) return "Gemini Flash-Lite";
  if (name.startsWith("gemini-2.5-flash")) return "Gemini 2.5 Flash";
  return name.replace(/[-_]/g, " ");
}
