<script lang="ts">
  /**
   * Today's provider quota. Lived in the sidebar until v3.5 (Home is a
   * notebook, not a dashboard); now rendered on Insights. Logic is the old
   * sidebar meter verbatim: Deepgram shows cumulative credit spend, Groq shows
   * its free-tier day buckets, other providers show a count with no fake fill.
   */
  import { onMount } from "svelte";
  import { usageStore } from "./usage-store.svelte";
  import { settings } from "./settings-store.svelte";
  import { type ModelUsage } from "./api";
  import { DEEPGRAM_FREE_CREDIT_USD } from "./provider-options";

  onMount(() => {
    usageStore.subscribe();
  });

  // Lightweight usage meters. Deepgram shows cumulative estimated spend
  // against the current free credit; model buckets show today's audio/tokens.
  function usageFor(stage: "stt" | "llm", provider: string, model: string): ModelUsage | null {
    const rows = usageStore.usage?.model_usage ?? [];
    const exact = rows.find((r) => r.stage === stage && r.provider === provider && r.model === model);
    if (exact) return exact;
    return rows.find((r) => r.stage === stage && r.provider === provider) ?? null;
  }

  function formatAudio(seconds = 0): string {
    if (seconds < 60) return `${Math.round(seconds)}s`;
    const minutes = seconds / 60;
    if (minutes < 10) return `${minutes.toFixed(1)}m`;
    return `${Math.round(minutes)}m`;
  }

  function formatTokens(tokens = 0): string {
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(tokens >= 10_000_000 ? 0 : 1)}M`;
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(tokens >= 10_000 ? 0 : 1)}k`;
    return String(tokens);
  }

  function formatCalls(calls = 0): string {
    return calls === 1 ? "1 call" : `${calls} calls`;
  }

  let currentSttUsage = $derived(usageFor("stt", settings.s.stt_provider, settings.s.stt_model));
  let currentLlmUsage = $derived(usageFor("llm", settings.s.llm_provider, settings.s.llm_model));
  let deepgramSpend = $derived(usageStore.usage?.deepgram_estimated_usd ?? 0);
  let deepgramCredit = $derived(usageStore.usage?.deepgram_free_credit_usd ?? DEEPGRAM_FREE_CREDIT_USD);
  let deepgramPct = $derived(Math.min(100, Math.round((deepgramSpend / deepgramCredit) * 100)));
  let countSttPct = $derived(Math.min(100, Math.round(((usageStore.usage?.stt_count ?? 0) / 2000) * 100)));
  let sttAudioPct = $derived(Math.min(100, Math.round(((currentSttUsage?.audio_seconds ?? 0) / 3600) * 100)));
  let sttPct = $derived(settings.s.stt_provider === "deepgram" ? deepgramPct : sttAudioPct || countSttPct);
  let llmTokenPct = $derived(Math.min(100, Math.round(((currentLlmUsage?.total_tokens ?? 0) / 200_000) * 100)));
  let llmCallPct = $derived(Math.min(100, Math.round(((currentLlmUsage?.calls ?? usageStore.usage?.llm_count ?? 0) / 1000) * 100)));
  let llmPct = $derived(llmTokenPct || llmCallPct);
  // The 2,000-call / 3,600s / 200k-token caps are Groq free-tier numbers.
  // Deepgram has its own credit meter. For every other provider the % fill
  // is meaningless, so show the number only (empty bar track, no fake fill).
  let sttHasMeter = $derived(
    settings.s.stt_provider === "groq" || settings.s.stt_provider === "deepgram",
  );
  let llmHasMeter = $derived(settings.s.llm_provider === "groq");
  // Deepgram's line shows lifetime credit spend, not a daily "today" number.
  let sttBarKey = $derived(settings.s.stt_provider === "deepgram" ? "Credit" : "STT");
  let sttUsageLabel = $derived(
    settings.s.stt_provider === "deepgram"
      ? `$${deepgramSpend.toFixed(2)}/$${Math.round(deepgramCredit)}`
      : currentSttUsage?.audio_seconds
        ? formatAudio(currentSttUsage.audio_seconds)
        : formatCalls(currentSttUsage?.calls ?? usageStore.usage?.stt_count ?? 0),
  );
  let llmUsageLabel = $derived(
    (currentLlmUsage?.total_tokens ?? 0) > 0
      ? `${formatTokens(currentLlmUsage?.total_tokens ?? 0)} tok`
      : formatCalls(currentLlmUsage?.calls ?? usageStore.usage?.llm_count ?? 0),
  );
  let sttUsageTitle = $derived(
    `${settings.s.stt_provider} / ${settings.s.stt_model}: ${formatCalls(currentSttUsage?.calls ?? 0)}, ${formatAudio(currentSttUsage?.audio_seconds ?? 0)} today`
  );
  let llmUsageTitle = $derived(
    `${settings.s.llm_provider} / ${settings.s.llm_model}: ${formatCalls(currentLlmUsage?.calls ?? 0)}, ${formatTokens(currentLlmUsage?.total_tokens ?? 0)} tokens today`
  );

  // Daily usage rolls over at UTC midnight. Show that moment in the
  // user's local timezone so they don't have to do mental UTC math
  // (especially relevant for IST users who are +5:30 from UTC).
  function nextUtcMidnightLocal(): string {
    const now = new Date();
    const next = new Date(Date.UTC(
      now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1, 0, 0, 0
    ));
    const hours = Math.floor((next.getTime() - now.getTime()) / 3_600_000);
    const mins = Math.floor((next.getTime() - now.getTime()) / 60_000) % 60;
    const timeStr = next.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    if (hours >= 1) return `resets at ${timeStr} (${hours}h ${mins}m)`;
    return `resets at ${timeStr} (${mins}m)`;
  }
  // Reactive, re-evaluates on each render. Cheap enough.
  let resetLabel = $state(nextUtcMidnightLocal());
  // Refresh every minute so the countdown stays accurate.
  onMount(() => {
    const t = setInterval(() => { resetLabel = nextUtcMidnightLocal(); }, 60_000);
    return () => clearInterval(t);
  });
  function pctClass(p: number): string {
    if (p < 50) return "ok";
    if (p < 85) return "warn";
    return "danger";
  }

</script>

<div class="footer-block">
  <div class="footer-title">Today's quota</div>
  <div class="footer-reset" title="Model buckets reset at midnight UTC. Deepgram credit spend stays cumulative.">
    {resetLabel}
  </div>

  <div class="bar-row">
    <span class="bar-key">{sttBarKey}</span>
    <div class="bar-track">
      {#if sttHasMeter}
        <div class="bar-fill {pctClass(sttPct)}" style="width: {sttPct}%"></div>
      {/if}
    </div>
    <span
      class="bar-val"
      title={settings.s.stt_provider === "deepgram"
        ? `${sttUsageTitle}. Deepgram estimate: $${deepgramSpend.toFixed(2)} used at $${(usageStore.usage?.deepgram_rate_usd_per_min ?? 0.0092).toFixed(4)}/min`
        : sttUsageTitle}
    >{sttUsageLabel}</span>
  </div>

  <div class="bar-row">
    <span class="bar-key">LLM</span>
    <div class="bar-track">
      {#if llmHasMeter}
        <div class="bar-fill {pctClass(llmPct)}" style="width: {llmPct}%"></div>
      {/if}
    </div>
    <span class="bar-val" title={llmUsageTitle}>{llmUsageLabel}</span>
  </div>
</div>

<style>
  .footer-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .footer-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .footer-reset {
    font-size: 10px;
    color: var(--text-secondary);
    margin-top: -2px;
    margin-bottom: 4px;
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }
  .bar-row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) max-content;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .bar-key {
    font-weight: 500;
    color: var(--text-secondary);
  }

  .bar-track {
    height: 6px;
    background: var(--bg-subtle);
    border-radius: 3px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 300ms ease, background 200ms ease;
  }

  .bar-fill.ok { background: var(--success); }
  .bar-fill.warn { background: var(--warning); }
  .bar-fill.danger { background: var(--danger); }

  .bar-val {
    font-variant-numeric: tabular-nums;
    color: var(--text-primary);
    font-size: 11px;
    max-width: 76px;
    overflow: hidden;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .usage-stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
    width: 100%;
  }

  .usage-chip {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .usage-chip.ok { background: var(--success-fade); color: var(--success); }
  .usage-chip.warn { background: var(--warning-fade); color: var(--warning); }
  .usage-chip.danger { background: var(--danger-fade); color: var(--danger); }

  .usage-chip-key {
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.04em;
    margin-bottom: 2px;
    opacity: 0.85;
  }

  .usage-chip-val {
    font-size: 11px;
    font-weight: 600;
  }
</style>
