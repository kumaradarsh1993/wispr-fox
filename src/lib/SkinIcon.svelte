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
    <ellipse cx="35" cy="60" rx="22" ry="4" fill="#87CEEB" opacity="0.5"/>
    <!-- Body — pear shape, symmetric -->
    <path d="M 35 36 C 50 36, 54 50, 52 58 C 46 64, 24 64, 18 58 C 16 50, 20 36, 35 36 Z"
          fill="url(#duck-body-{size})" stroke="#D4A800" stroke-width="1"/>
    <!-- Head — centered oval -->
    <ellipse cx="35" cy="24" rx="13" ry="12" fill="#FFE066" stroke="#D4A800" stroke-width="1"/>
    <!-- Beak — CENTERED below eyes (front-facing) -->
    <path d="M 27 32 Q 35 28, 43 32 Q 35 36, 27 32 Z" fill="#FF8C00" stroke="#CC7000" stroke-width="0.7"/>
    <!-- Eyes — symmetric flanking the beak -->
    <ellipse cx="29" cy="22" rx="2.4" ry="3" fill="#fff" stroke="#1d1d1f" stroke-width="0.8"/>
    <circle cx="29" cy="22.5" r="1.2" fill="#1d1d1f"/>
    <ellipse cx="41" cy="22" rx="2.4" ry="3" fill="#fff" stroke="#1d1d1f" stroke-width="0.8"/>
    <circle cx="41" cy="22.5" r="1.2" fill="#1d1d1f"/>
  </svg>
{:else if skin === "cat"}
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <defs>
      <linearGradient id="cat-body-{size}" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="#FF9F4A"/>
        <stop offset="100%" stop-color="#D9651A"/>
      </linearGradient>
    </defs>
    <!-- Body — orange tabby -->
    <ellipse cx="35" cy="48" rx="18" ry="13" fill="url(#cat-body-{size})" stroke="#A04510" stroke-width="0.9"/>
    <!-- White belly -->
    <ellipse cx="33" cy="52" rx="10" ry="6" fill="#FFFAF0" opacity="0.9"/>
    <!-- Body stripes -->
    <path d="M 42 40 Q 46 44, 48 50" stroke="#A04510" stroke-width="1.1" fill="none" stroke-linecap="round" opacity="0.7"/>
    <path d="M 24 40 Q 20 44, 19 50" stroke="#A04510" stroke-width="1.1" fill="none" stroke-linecap="round" opacity="0.7"/>
    <!-- Tail with stripes -->
    <path d="M 52 48 C 58 40, 60 52, 55 56" fill="none" stroke="#D9651A" stroke-width="2.6" stroke-linecap="round"/>
    <path d="M 56 45 L 59 44" stroke="#A04510" stroke-width="0.9" stroke-linecap="round" opacity="0.8"/>
    <!-- Head — orange -->
    <circle cx="35" cy="30" r="12" fill="#FFB066" stroke="#A04510" stroke-width="0.8"/>
    <!-- Forehead "M" stripes -->
    <path d="M 30 24 L 32 28" stroke="#A04510" stroke-width="0.9" stroke-linecap="round" opacity="0.75"/>
    <path d="M 35 23 L 35 28" stroke="#A04510" stroke-width="0.9" stroke-linecap="round" opacity="0.75"/>
    <path d="M 40 24 L 38 28" stroke="#A04510" stroke-width="0.9" stroke-linecap="round" opacity="0.75"/>
    <!-- White muzzle -->
    <ellipse cx="35" cy="35" rx="6" ry="4" fill="#FFFAF0" opacity="0.95"/>
    <!-- Ears -->
    <path d="M 24 24 L 22 12 L 30 20 Z" fill="#FFB066" stroke="#A04510" stroke-width="0.8"/>
    <path d="M 25 21 L 24 15 L 29 20 Z" fill="#FF9999" opacity="0.6"/>
    <path d="M 46 24 L 48 12 L 40 20 Z" fill="#FFB066" stroke="#A04510" stroke-width="0.8"/>
    <path d="M 45 21 L 46 15 L 41 20 Z" fill="#FF9999" opacity="0.6"/>
    <!-- Eyes -->
    <ellipse cx="30" cy="29" rx="3" ry="3.5" fill="#7FFF00" stroke="#1a1a1a" stroke-width="0.8"/>
    <ellipse cx="30" cy="29.5" rx="1" ry="2" fill="#1a1a1a"/>
    <ellipse cx="40" cy="29" rx="3" ry="3.5" fill="#7FFF00" stroke="#1a1a1a" stroke-width="0.8"/>
    <ellipse cx="40" cy="29.5" rx="1" ry="2" fill="#1a1a1a"/>
    <!-- Nose -->
    <path d="M 34 33 L 35 35 L 36 33 Z" fill="#FF6B6B"/>
    <!-- Whiskers -->
    <line x1="18" y1="33" x2="28" y2="34" stroke="#FFF8E0" stroke-width="0.6"/>
    <line x1="42" y1="34" x2="52" y2="33" stroke="#FFF8E0" stroke-width="0.6"/>
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
