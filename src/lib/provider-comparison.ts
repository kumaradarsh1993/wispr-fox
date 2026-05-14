// Cloud-provider comparison matrix used by onboarding's "how is this free?"
// explainer and any future docs/about pages. Lives outside Settings so the
// Settings UI stops being a documentation page.
//
// Re-verify every 2-3 months as free tiers shift fast.
// Sources: Vellum, llm-stats, costgoat, pecollective, getaiperks (May 2026).

export type ProviderStatus = "active" | "planned" | "considered";

export type ProviderRow = {
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
  status: ProviderStatus;
};

export const PROVIDERS: ProviderRow[] = [
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
