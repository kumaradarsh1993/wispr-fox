<script lang="ts">
  import { onMount } from "svelte";
  import { onFlowState, type FlowState } from "./api";

  let state = $state<FlowState>("idle");

  onMount(() => {
    let unlisten: (() => void) | undefined;
    onFlowState((s) => (state = s)).then((u) => (unlisten = u));
    return () => unlisten?.();
  });

  const labelByState: Record<FlowState, string> = {
    idle: "Ready",
    recording: "Recording…",
    transcribing: "Transcribing…",
    denoising: "Clearing noise…",
    cleaning: "Cleaning up…",
    injecting: "Pasting…",
  };
</script>

<!-- Tone comes from theme tokens (see app.css), never from Tailwind palette
     classes — the old version stayed grey/red/blue in dark and retro. -->
<span class="badge" data-state={state} aria-live="polite">
  <span class="dot"></span>
  {labelByState[state]}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: var(--radius-pill);
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 1.4;
    color: var(--text-secondary);
    background: var(--bg-subtle);
    transition: background var(--motion-base) var(--ease-standard),
      color var(--motion-base) var(--ease-standard);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .badge[data-state="recording"] {
    color: var(--danger);
    background: var(--danger-fade);
  }
  .badge[data-state="recording"] .dot {
    animation: pulse 1.1s ease-in-out infinite;
  }
  .badge[data-state="transcribing"],
  .badge[data-state="denoising"] {
    color: var(--info);
    background: var(--info-fade);
  }
  .badge[data-state="cleaning"] {
    color: var(--mode-draft);
    background: var(--mode-draft-fade);
  }
  .badge[data-state="injecting"] {
    color: var(--success);
    background: var(--success-fade);
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }
</style>
