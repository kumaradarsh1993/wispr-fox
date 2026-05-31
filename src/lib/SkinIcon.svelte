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
{:else if skin === "duck"}
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <defs>
      <linearGradient id="duck-body-{size}" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="#FFE066"/>
        <stop offset="100%" stop-color="#FFD700"/>
      </linearGradient>
    </defs>
    <!-- Water -->
    <ellipse cx="35" cy="58" rx="22" ry="5" fill="#87CEEB" opacity="0.5"/>
    <!-- Body -->
    <ellipse cx="35" cy="44" rx="18" ry="16" fill="url(#duck-body-{size})" stroke="#D4A800" stroke-width="1.2"/>
    <!-- Head -->
    <circle cx="35" cy="26" r="11" fill="#FFE066" stroke="#D4A800" stroke-width="1"/>
    <!-- Beak -->
    <path d="M 40 28 L 50 26 L 40 30 Z" fill="#FF8C00" stroke="#CC7000" stroke-width="0.8"/>
    <!-- Eyes -->
    <ellipse cx="32" cy="24" rx="2.5" ry="3" fill="#fff" stroke="#1d1d1f" stroke-width="0.8"/>
    <circle cx="32" cy="25" r="1.2" fill="#1d1d1f"/>
    <circle cx="31.4" cy="24" r="0.5" fill="#fff" opacity="0.9"/>
  </svg>
{:else if skin === "cat"}
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <!-- Body -->
    <ellipse cx="35" cy="48" rx="18" ry="14" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="1"/>
    <!-- Head -->
    <circle cx="35" cy="30" r="12" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="0.8"/>
    <!-- Ears -->
    <path d="M 24 24 L 22 12 L 30 20 Z" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="0.8"/>
    <path d="M 25 21 L 24 15 L 29 20 Z" fill="#FF9999" opacity="0.6"/>
    <path d="M 46 24 L 48 12 L 40 20 Z" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="0.8"/>
    <path d="M 45 21 L 46 15 L 41 20 Z" fill="#FF9999" opacity="0.6"/>
    <!-- Eyes -->
    <ellipse cx="30" cy="29" rx="3" ry="3.5" fill="#7FFF00" stroke="#1a1a1a" stroke-width="0.8"/>
    <ellipse cx="30" cy="29.5" rx="1" ry="2" fill="#1a1a1a"/>
    <ellipse cx="40" cy="29" rx="3" ry="3.5" fill="#7FFF00" stroke="#1a1a1a" stroke-width="0.8"/>
    <ellipse cx="40" cy="29.5" rx="1" ry="2" fill="#1a1a1a"/>
    <!-- Nose -->
    <path d="M 34 33 L 35 35 L 36 33 Z" fill="#FF6B6B"/>
    <!-- Whiskers -->
    <line x1="18" y1="32" x2="28" y2="33" stroke="#666" stroke-width="0.6"/>
    <line x1="18" y1="36" x2="28" y2="35" stroke="#666" stroke-width="0.6"/>
    <line x1="42" y1="33" x2="52" y2="32" stroke="#666" stroke-width="0.6"/>
    <line x1="42" y1="35" x2="52" y2="36" stroke="#666" stroke-width="0.6"/>
    <!-- Tail -->
    <path d="M 52 48 C 58 40, 60 52, 55 56" fill="none" stroke="#2B2B2B" stroke-width="3" stroke-linecap="round"/>
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
