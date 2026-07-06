<script lang="ts">
  import type { Skin } from "./skin-store.svelte";
  import { rasterAvatarForSkin } from "./avatar-packs";
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
  let rasterPack = $derived(rasterAvatarForSkin(skin));
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
{:else if rasterPack}
  <img
    src={rasterPack.thumbnail}
    alt=""
    width={size}
    height={size}
    style="object-fit: contain; filter: drop-shadow(0 1px 2px rgba(0,0,0,0.18));"
  />
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
{:else if skin.startsWith("pet-")}
  <!-- Terminal pets — the sheet's first idle frame, clipped by the viewBox
       (the <image> is laid out at full sheet size; only frame 0 shows). -->
  <svg viewBox="0 0 192 208" width={size} height={size} aria-hidden="true">
    <image href="/pets/{skin.slice(4)}.webp" x="0" y="0" width="1536" height="1872" />
  </svg>
{:else if skin === "wave"}
  <!-- Wave bar — five rounded waveform bars, currentColor so the icon
       follows active/hover accents like the nav glyphs do. -->
  <svg viewBox="0 0 48 48" width={size} height={size} aria-hidden="true">
    <g stroke="currentColor" stroke-width="4.5" stroke-linecap="round" fill="none">
      <path d="M 8 20 V 28" />
      <path d="M 16 14 V 34" />
      <path d="M 24 9 V 39" />
      <path d="M 32 16 V 32" />
      <path d="M 40 21 V 27" />
    </g>
  </svg>
{:else if skin === "siri"}
  <!-- Siri orb — a small multicolour gradient sphere with a gloss highlight. -->
  <svg viewBox="0 0 48 48" width={size} height={size} aria-hidden="true">
    <defs>
      <radialGradient id="ski-siri" cx="0.5" cy="0.5" r="0.62">
        <stop offset="0%" stop-color="#5ac8fa"/>
        <stop offset="42%" stop-color="#a06bff"/>
        <stop offset="72%" stop-color="#ff5fa2"/>
        <stop offset="100%" stop-color="#ff8a4c"/>
      </radialGradient>
      <radialGradient id="ski-siri-gloss" cx="0.36" cy="0.28" r="0.4">
        <stop offset="0%" stop-color="#ffffff" stop-opacity="0.85"/>
        <stop offset="100%" stop-color="#ffffff" stop-opacity="0"/>
      </radialGradient>
    </defs>
    <circle cx="24" cy="24" r="16" fill="url(#ski-siri)"/>
    <ellipse cx="18" cy="16" rx="8" ry="6" fill="url(#ski-siri-gloss)"/>
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
