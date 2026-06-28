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
{:else if skin === "cat-lab"}
  <!-- Experimental cat — same silhouette as Desk Cat, with thin white edge
       highlights and lighter accent tones so features read on dark surfaces. -->
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <defs>
      <linearGradient id="ski-cl-body" x1="0.3" y1="0" x2="0.7" y2="1">
        <stop offset="0%" stop-color="#3D3D3D"/>
        <stop offset="100%" stop-color="#222"/>
      </linearGradient>
    </defs>
    <!-- Tail with edge highlight -->
    <path d="M 52 48 C 58 40, 60 52, 55 56" fill="none" stroke="#ffffff" stroke-width="3.4" stroke-linecap="round" opacity="0.5"/>
    <path d="M 52 48 C 58 40, 60 52, 55 56" fill="none" stroke="#2B2B2B" stroke-width="2.2" stroke-linecap="round"/>
    <!-- Body -->
    <ellipse cx="35" cy="48" rx="18" ry="14" fill="url(#ski-cl-body)" stroke="#ffffff" stroke-width="0.9" stroke-opacity="0.55"/>
    <!-- Belly highlight -->
    <ellipse cx="32" cy="51" rx="10" ry="6" fill="#5a5a5a" opacity="0.6"/>
    <!-- Head -->
    <circle cx="35" cy="30" r="12" fill="url(#ski-cl-body)" stroke="#ffffff" stroke-width="0.9" stroke-opacity="0.55"/>
    <!-- Ears -->
    <path d="M 24 24 L 22 12 L 30 20 Z" fill="#2B2B2B" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.55"/>
    <path d="M 25 21 L 24 15 L 29 20 Z" fill="#FF9999" opacity="0.7"/>
    <path d="M 46 24 L 48 12 L 40 20 Z" fill="#2B2B2B" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.55"/>
    <path d="M 45 21 L 46 15 L 41 20 Z" fill="#FF9999" opacity="0.7"/>
    <!-- Eyes -->
    <ellipse cx="30" cy="29" rx="3" ry="3.5" fill="#7FFF00" stroke="#1a1a1a" stroke-width="0.8"/>
    <ellipse cx="30" cy="29.5" rx="1" ry="2" fill="#1a1a1a"/>
    <ellipse cx="40" cy="29" rx="3" ry="3.5" fill="#7FFF00" stroke="#1a1a1a" stroke-width="0.8"/>
    <ellipse cx="40" cy="29.5" rx="1" ry="2" fill="#1a1a1a"/>
    <!-- Nose + mouth (lighter so visible) -->
    <path d="M 34 33 L 35 35 L 36 33 Z" fill="#FF6B6B"/>
    <path d="M 32 36 Q 35 38 38 36" fill="none" stroke="#d0d0d0" stroke-width="0.7" stroke-linecap="round"/>
    <!-- Whiskers -->
    <line x1="18" y1="32" x2="28" y2="33" stroke="#aaa" stroke-width="0.6"/>
    <line x1="18" y1="36" x2="28" y2="35" stroke="#aaa" stroke-width="0.6"/>
    <line x1="42" y1="33" x2="52" y2="32" stroke="#aaa" stroke-width="0.6"/>
    <line x1="42" y1="35" x2="52" y2="36" stroke="#aaa" stroke-width="0.6"/>
  </svg>
{:else if skin === "duo"}
  <!-- Khaumani & Indy — two heads: serene white (left), eager orange (right). -->
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <!-- White cat head (back-left, calm) -->
    <g>
      <path d="M 14 26 L 11 12 L 24 22 Z" fill="#fbfaf6" stroke="#c9c2b2" stroke-width="1"/>
      <path d="M 34 24 L 39 11 L 26 21 Z" fill="#fbfaf6" stroke="#c9c2b2" stroke-width="1"/>
      <path d="M 15 23 L 14 16 L 21 21 Z" fill="#f2c6c2"/>
      <path d="M 33 21 L 36 15 L 28 20 Z" fill="#f2c6c2"/>
      <circle cx="24" cy="32" r="14" fill="#fbfaf6" stroke="#c9c2b2" stroke-width="1.1"/>
      <!-- Heavy-lidded calm eyes -->
      <path d="M 16.5 31 Q 19 33 21.5 31" fill="none" stroke="#5d4430" stroke-width="1.6" stroke-linecap="round"/>
      <path d="M 26.5 31 Q 29 33 31.5 31" fill="none" stroke="#5d4430" stroke-width="1.6" stroke-linecap="round"/>
      <path d="M 22.8 37 L 24 39 L 25.2 37 Z" fill="#eda4a4"/>
    </g>
    <!-- Orange tabby head (front-right, alert) -->
    <g>
      <path d="M 36 44 L 31 26 L 48 39 Z" fill="#ef9c4e" stroke="#c96f28" stroke-width="1"/>
      <path d="M 60 44 L 65 26 L 48 39 Z" fill="#ef9c4e" stroke="#c96f28" stroke-width="1"/>
      <path d="M 38 40 L 36 30 L 46 38 Z" fill="#f0b3a4"/>
      <path d="M 58 40 L 60 30 L 50 38 Z" fill="#f0b3a4"/>
      <circle cx="48" cy="50" r="15" fill="#f3a85f" stroke="#c96f28" stroke-width="1.1"/>
      <path d="M 43 38 L 42.6 43 M 48 37 L 48 42.5 M 53 38 L 53.4 43" fill="none" stroke="#cf7427" stroke-width="1.6" stroke-linecap="round"/>
      <ellipse cx="48" cy="56" rx="8" ry="5.5" fill="#fdf6ec"/>
      <circle cx="42.5" cy="48.5" r="3" fill="#b97f33" stroke="#7a4d1c" stroke-width="0.6"/>
      <circle cx="42.5" cy="48.8" r="1.5" fill="#1c1208"/>
      <circle cx="53.5" cy="48.5" r="3" fill="#b97f33" stroke="#7a4d1c" stroke-width="0.6"/>
      <circle cx="53.5" cy="48.8" r="1.5" fill="#1c1208"/>
      <path d="M 46.8 53.5 L 48 55.5 L 49.2 53.5 Z" fill="#e58f86"/>
    </g>
  </svg>
{:else if skin === "duo-hd"}
  <!-- Khaumani & Indy ✦ (remastered) — gradient fur + a sparkle to mark the
       higher-fidelity, livelier version. Same two-head read as the duo icon. -->
  <svg viewBox="0 0 70 70" width={size} height={size} aria-hidden="true">
    <defs>
      <linearGradient id="ski-hd-white" x1="0.5" y1="0" x2="0.5" y2="1">
        <stop offset="0%" stop-color="#ffffff"/>
        <stop offset="100%" stop-color="#e6e0d0"/>
      </linearGradient>
      <linearGradient id="ski-hd-orange" x1="0.4" y1="0" x2="0.6" y2="1">
        <stop offset="0%" stop-color="#f8b76b"/>
        <stop offset="100%" stop-color="#dd8030"/>
      </linearGradient>
      <radialGradient id="ski-hd-amber" cx="0.5" cy="0.4" r="0.6">
        <stop offset="0%" stop-color="#ffd57e"/>
        <stop offset="100%" stop-color="#b3711f"/>
      </radialGradient>
    </defs>
    <!-- White cat head (back-left, calm) -->
    <g>
      <path d="M 14 26 L 11 12 L 24 22 Z" fill="url(#ski-hd-white)" stroke="#c9c2b2" stroke-width="1"/>
      <path d="M 34 24 L 39 11 L 26 21 Z" fill="url(#ski-hd-white)" stroke="#c9c2b2" stroke-width="1"/>
      <path d="M 15 23 L 14 16 L 21 21 Z" fill="#f3c9c5"/>
      <path d="M 33 21 L 36 15 L 28 20 Z" fill="#f3c9c5"/>
      <circle cx="24" cy="32" r="14" fill="url(#ski-hd-white)" stroke="#c9c2b2" stroke-width="1.1"/>
      <path d="M 16.5 31 Q 19 33 21.5 31" fill="none" stroke="#5d7a3c" stroke-width="1.6" stroke-linecap="round"/>
      <path d="M 26.5 31 Q 29 33 31.5 31" fill="none" stroke="#5d7a3c" stroke-width="1.6" stroke-linecap="round"/>
      <path d="M 22.8 37 L 24 39 L 25.2 37 Z" fill="#eda4a4"/>
    </g>
    <!-- Orange tabby head (front-right, alert) -->
    <g>
      <path d="M 36 44 L 31 26 L 48 39 Z" fill="url(#ski-hd-orange)" stroke="#c96f28" stroke-width="1"/>
      <path d="M 60 44 L 65 26 L 48 39 Z" fill="url(#ski-hd-orange)" stroke="#c96f28" stroke-width="1"/>
      <path d="M 38 40 L 36 30 L 46 38 Z" fill="#eeb3a1"/>
      <path d="M 58 40 L 60 30 L 50 38 Z" fill="#eeb3a1"/>
      <circle cx="48" cy="50" r="15" fill="url(#ski-hd-orange)" stroke="#c96f28" stroke-width="1.1"/>
      <path d="M 43 38 L 42.6 43 M 48 37 L 48 42.5 M 53 38 L 53.4 43" fill="none" stroke="#c9701f" stroke-width="1.6" stroke-linecap="round"/>
      <ellipse cx="48" cy="56" rx="8" ry="5.5" fill="#fdf3e2"/>
      <circle cx="42.5" cy="48.5" r="3.1" fill="url(#ski-hd-amber)" stroke="#7a4d1c" stroke-width="0.6"/>
      <circle cx="42.5" cy="48.8" r="1.5" fill="#1c1208"/>
      <circle cx="53.5" cy="48.5" r="3.1" fill="url(#ski-hd-amber)" stroke="#7a4d1c" stroke-width="0.6"/>
      <circle cx="53.5" cy="48.8" r="1.5" fill="#1c1208"/>
      <path d="M 46.8 53.5 L 48 55.5 L 49.2 53.5 Z" fill="#e58f86"/>
    </g>
    <!-- Sparkle mark -->
    <path d="M 60 12 L 61.4 16 L 65.4 17.4 L 61.4 18.8 L 60 22.8 L 58.6 18.8 L 54.6 17.4 L 58.6 16 Z" fill="#eec25a"/>
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
