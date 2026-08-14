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

  const colorByState: Record<FlowState, string> = {
    idle: "bg-gray-200 text-gray-700",
    recording: "bg-red-100 text-red-700 animate-pulse",
    transcribing: "bg-blue-100 text-blue-700",
    denoising: "bg-blue-100 text-blue-700",
    cleaning: "bg-purple-100 text-purple-700",
    injecting: "bg-green-100 text-green-700",
  };
</script>

<span
  class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium {colorByState[state]}"
>
  <span class="w-1.5 h-1.5 rounded-full bg-current"></span>
  {labelByState[state]}
</span>
