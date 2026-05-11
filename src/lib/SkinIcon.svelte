<script lang="ts">
  import type { Skin } from "./skin-store.svelte";
  let { skin, size = 28 }: { skin: Skin; size?: number } = $props();
</script>

{#if skin === "off"}
  <svg viewBox="0 0 80 80" width={size} height={size} aria-hidden="true">
    <circle cx="40" cy="40" r="28" fill="none" stroke="currentColor" stroke-width="3" stroke-dasharray="4 4" opacity="0.65"/>
    <line x1="20" y1="20" x2="60" y2="60" stroke="currentColor" stroke-width="3" opacity="0.65"/>
  </svg>
{:else if skin === "stylized"}
  <svg viewBox="0 0 60 80" width={size * 0.75} height={size} aria-hidden="true">
    <path d="M 25 14 C 25 8, 35 8, 35 14 L 35 56 C 35 67, 19 67, 19 56 L 19 26 C 19 20, 29 20, 29 26 L 29 51"
          fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M 18 18 Q 21 16, 24 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
    <path d="M 30 18 Q 33 16, 36 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
    <ellipse cx="22" cy="25" rx="3" ry="3.5" fill="var(--bg-card)" stroke="currentColor" stroke-width="1.2"/>
    <circle cx="22" cy="26" r="1.4" fill="currentColor"/>
    <ellipse cx="32" cy="25" rx="3" ry="3.5" fill="var(--bg-card)" stroke="currentColor" stroke-width="1.2"/>
    <circle cx="32" cy="26" r="1.4" fill="currentColor"/>
  </svg>
{:else if skin === "real-clippy"}
  <svg viewBox="0 0 70 80" width={size * 0.85} height={size} aria-hidden="true">
    <path d="M 27 12 C 27 6, 43 6, 43 12 L 43 64 C 43 74, 19 74, 19 60 L 19 28 C 19 20, 33 20, 33 28 L 33 56"
          fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M 17 20 Q 22 16, 28 20 L 27 22 Q 22 19, 18 22 Z" fill="currentColor"/>
    <path d="M 30 20 Q 35 16, 41 20 L 40 22 Q 35 19, 31 22 Z" fill="currentColor"/>
    <ellipse cx="23" cy="28" rx="4" ry="4.5" fill="var(--bg-card)" stroke="currentColor" stroke-width="1.4"/>
    <ellipse cx="23" cy="29" rx="1.5" ry="2" fill="currentColor"/>
    <ellipse cx="37" cy="28" rx="4" ry="4.5" fill="var(--bg-card)" stroke="currentColor" stroke-width="1.4"/>
    <ellipse cx="37" cy="29" rx="1.5" ry="2" fill="currentColor"/>
  </svg>
{:else if skin === "chippy"}
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <defs>
      <linearGradient id="chip-{skin}-{size}" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="#fbe3a0"/>
        <stop offset="100%" stop-color="#b6792a"/>
      </linearGradient>
    </defs>
    <path d="M 12 35 C 8 18, 30 10, 35 20 C 40 10, 62 18, 58 35 C 62 52, 40 60, 35 50 C 30 60, 8 52, 12 35 Z"
          fill="url(#chip-{skin}-{size})" stroke="#7a4d12" stroke-width="1.5"/>
    <path d="M 14 38 Q 35 30, 56 38" fill="none" stroke="#a06010" stroke-width="1" opacity="0.5"/>
    <ellipse cx="28" cy="38" rx="3" ry="3.5" fill="#fff" stroke="#4a2208" stroke-width="1"/>
    <circle cx="28" cy="39" r="1.4" fill="#1d1d1f"/>
    <ellipse cx="42" cy="38" rx="3" ry="3.5" fill="#fff" stroke="#4a2208" stroke-width="1"/>
    <circle cx="42" cy="39" r="1.4" fill="#1d1d1f"/>
    <path d="M 32 46 Q 35 49, 38 46" fill="none" stroke="#4a2208" stroke-width="1.4" stroke-linecap="round"/>
  </svg>
{/if}
