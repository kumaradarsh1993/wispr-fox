<script lang="ts">
  import type { Skin } from "./skin-store.svelte";
  // Import the actual Microsoft Clippy sprite data (base64 PNG + frame map)
  // from the vendored clippyjs agent file. Vite bundles it once and dedupes
  // across all SkinIcon usages.
  import clippyAgent from "./clippyjs-vendor/clippy-agent.js";

  let { skin, size = 28 }: { skin: Skin; size?: number } = $props();

  // Hand-picked frame from the agent's "Writing" animation — Clippy
  // bent over the yellow notepad, the iconic "Clippy reading" pose
  // that matches what users remember from MS Office. Coords are the
  // top-left of the 124x93 frame within the sprite sheet.
  const CLIPPY_FRAME = { x: 992, y: 1953, w: 124, h: 93 };

  // Final on-screen frame size for the sprite preview — taller than `size`
  // to preserve the original 124x93 aspect ratio.
  let spriteW = $derived(Math.round(size * (CLIPPY_FRAME.w / CLIPPY_FRAME.h)));
  let spriteH = $derived(size);
  let scale = $derived(size / CLIPPY_FRAME.h);
</script>

{#if skin === "off"}
  <svg viewBox="0 0 80 80" width={size} height={size} aria-hidden="true">
    <circle cx="40" cy="40" r="28" fill="none" stroke="currentColor" stroke-width="3" stroke-dasharray="4 4" opacity="0.65"/>
    <line x1="20" y1="20" x2="60" y2="60" stroke="currentColor" stroke-width="3" opacity="0.65"/>
  </svg>
{:else if skin === "fox"}
  <!-- Watercolor fox favicon — the bold flat face from the asset pack
       (different from the watercolor fox-logo so it reads clearly at
       small picker sizes). -->
  <img src="/fox/fox-favicon.png" alt="" width={size} height={size} style="object-fit: contain;" />
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
{:else if skin === "beige"}
  <!-- Same shape as stylized, but cream outline + warm-brown features so
       the picker preview hints at the theme inversion. -->
  <svg viewBox="0 0 60 80" width={size * 0.75} height={size} aria-hidden="true">
    <path d="M 25 14 C 25 8, 35 8, 35 14 L 35 56 C 35 67, 19 67, 19 56 L 19 26 C 19 20, 29 20, 29 26 L 29 51"
          fill="none" stroke="#d8c89e" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M 18 18 Q 21 16, 24 18" fill="none" stroke="#8a5a2a" stroke-width="2.2" stroke-linecap="round"/>
    <path d="M 30 18 Q 33 16, 36 18" fill="none" stroke="#8a5a2a" stroke-width="2.2" stroke-linecap="round"/>
    <ellipse cx="22" cy="25" rx="3.2" ry="3.7" fill="#fff9ec" stroke="#6b3a0e" stroke-width="1.3"/>
    <circle cx="22" cy="26" r="1.6" fill="#3a1a02"/>
    <ellipse cx="32" cy="25" rx="3.2" ry="3.7" fill="#fff9ec" stroke="#6b3a0e" stroke-width="1.3"/>
    <circle cx="32" cy="26" r="1.6" fill="#3a1a02"/>
  </svg>
{:else if skin === "real-clippy"}
  <!-- The ACTUAL Microsoft Clippy frame from the vendored sprite. -->
  <div
    class="clippy-sprite-wrap"
    style="width: {spriteW}px; height: {spriteH}px;"
    aria-hidden="true"
  >
    <div
      class="clippy-sprite-frame"
      style="
        width: {CLIPPY_FRAME.w}px;
        height: {CLIPPY_FRAME.h}px;
        background: url('{clippyAgent.image}') -{CLIPPY_FRAME.x}px -{CLIPPY_FRAME.y}px no-repeat;
        transform: scale({scale});
        transform-origin: top left;
      "
    ></div>
  </div>
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

<style>
  .clippy-sprite-wrap {
    display: inline-block;
    overflow: hidden;
    line-height: 0;
  }
  .clippy-sprite-frame {
    image-rendering: -webkit-optimize-contrast;
  }
</style>
