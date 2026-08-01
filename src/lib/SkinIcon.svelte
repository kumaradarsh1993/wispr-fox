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
  let petSheetHeight = $derived(skin === "pet-mochi-marmalade" ? 2288 : 1872);
</script>

{#if skin === "off"}
  <svg viewBox="0 0 80 80" width={size} height={size} aria-hidden="true">
    <circle cx="40" cy="40" r="28" fill="none" stroke="currentColor" stroke-width="3" stroke-dasharray="4 4" opacity="0.65"/>
    <line x1="20" y1="20" x2="60" y2="60" stroke="currentColor" stroke-width="3" opacity="0.65"/>
  </svg>
{:else if skin === "fox"}
  <!-- The actual idle floater pose, not a separate app-mark illustration. -->
  <img class="fox-preview" src="/fox/fox-sitting.png" alt="" width={size} height={size} />
{:else if rasterPack}
  <img
    src={rasterPack.thumbnail}
    alt=""
    width={size}
    height={size}
    style="object-fit: contain; filter: drop-shadow(0 1px 2px rgba(0,0,0,0.18));"
  />
{:else if skin === "stylized"}
  <svg class="clippo-preview" viewBox="22 12 66 126" width={size * 0.74} height={size} aria-hidden="true">
    <path d="M 50 30 C 50 18, 70 18, 70 30 L 70 110 C 70 132, 38 132, 38 110 L 38 50 C 38 38, 58 38, 58 50 L 58 100"
      fill="none" stroke="#1d1d1f" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="M 36 36 Q 42 32, 48 36 M 60 36 Q 66 32, 72 36" fill="none" stroke="#1d1d1f" stroke-width="3.5" stroke-linecap="round"/>
    <ellipse cx="44" cy="51" rx="8.5" ry="9.5" fill="#fff" stroke="#1d1d1f" stroke-width="2.2"/>
    <circle cx="46" cy="53" r="4" fill="#1d1d1f"/><circle cx="44.6" cy="51.2" r="1.1" fill="#fff"/>
    <ellipse cx="66" cy="51" rx="8.5" ry="9.5" fill="#fff" stroke="#1d1d1f" stroke-width="2.2"/>
    <circle cx="68" cy="53" r="4" fill="#1d1d1f"/><circle cx="66.6" cy="51.2" r="1.1" fill="#fff"/>
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
  <svg class="blacky-preview" viewBox="12 34 108 126" width={size} height={size} aria-hidden="true">
    <path d="M 103 135 C 125 116, 124 91, 111 83 C 101 77, 94 87, 106 94" fill="none" stroke="#2b2b2b" stroke-width="8" stroke-linecap="round"/>
    <ellipse cx="65" cy="130" rx="45" ry="27" fill="#292929" stroke="#555" stroke-width="1.3"/>
    <ellipse cx="43" cy="148" rx="10" ry="6" fill="#454545"/><ellipse cx="84" cy="148" rx="10" ry="6" fill="#454545"/>
    <circle cx="65" cy="86" r="30" fill="#303030" stroke="#5a5a5a" stroke-width="1.2"/>
    <path d="M 40 72 L 31 40 L 53 64 Z M 90 72 L 99 40 L 77 64 Z" fill="#303030" stroke="#5a5a5a" stroke-width="1.2"/>
    <path d="M 42 66 L 36 48 L 49 63 Z M 88 66 L 94 48 L 81 63 Z" fill="#d77f91" opacity=".82"/>
    <ellipse cx="53" cy="84" rx="8" ry="9" fill="#8fe31f"/><ellipse cx="77" cy="84" rx="8" ry="9" fill="#8fe31f"/>
    <ellipse cx="53" cy="85" rx="1.8" ry="6.5" fill="#111"/><ellipse cx="77" cy="85" rx="1.8" ry="6.5" fill="#111"/>
    <circle cx="50.5" cy="81" r="1.6" fill="#fff"/><circle cx="74.5" cy="81" r="1.6" fill="#fff"/>
    <path d="M 62 95 L 65 99 L 68 95 Z" fill="#e87881"/>
    <path d="M 58 101 Q 65 106 72 101" fill="none" stroke="#cfcfcf" stroke-width="1.1" stroke-linecap="round"/>
    <g stroke="#a0a0a0" stroke-width="1" stroke-linecap="round"><path d="M 24 94 L 50 97 M 23 102 L 50 100 M 80 97 L 106 94 M 80 100 L 107 102"/></g>
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
    <image href="/pets/{skin.slice(4)}.webp" x="0" y="0" width="1536" height={petSheetHeight} />
  </svg>
{:else if skin === "wave"}
  <div class="wavy-preview" style="width: {size}px; height: {size}px;" aria-hidden="true">
    {#each [6, 12, 20, 14, 8, 16, 7] as h}
      <i style="height: {Math.max(2, h * size / 48)}px"></i>
    {/each}
  </div>
{:else if skin === "siri"}
  <div class="siri-preview" style="width: {size}px; height: {size}px;" aria-hidden="true"></div>
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

  .fox-preview {
    object-fit: contain;
    filter: drop-shadow(0 2px 3px rgba(90, 53, 20, 0.2));
  }

  .clippo-preview,
  .blacky-preview {
    overflow: visible;
    filter: drop-shadow(0 2px 2px rgba(0, 0, 0, 0.18));
  }

  .wavy-preview {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: max(1px, calc(var(--icon-size, 48px) / 34));
    box-sizing: border-box;
    padding: 0 12%;
    border: 1px solid rgba(255, 255, 255, 0.19);
    border-radius: 999px;
    background: linear-gradient(180deg, #46484d, #202126);
    box-shadow: 0 3px 7px rgba(0, 0, 0, 0.28), inset 0 1px rgba(255, 255, 255, 0.13);
  }

  .wavy-preview i {
    width: max(1px, 5%);
    min-height: 2px;
    border-radius: 999px;
    background: linear-gradient(180deg, #fff, #b9e8ff);
    box-shadow: 0 0 3px rgba(121, 211, 255, 0.7);
  }

  .siri-preview {
    position: relative;
    box-sizing: border-box;
    border: 2px solid rgba(255, 255, 255, 0.68);
    border-radius: 50%;
    background:
      radial-gradient(circle at 34% 25%, rgba(255,255,255,.94), transparent 19%),
      conic-gradient(from 205deg, #37d5ff, #6968ff, #d84dff, #ff4e9b, #ff9548, #45e0df, #37d5ff);
    box-shadow:
      0 0 0 2px rgba(112, 95, 255, 0.16),
      0 3px 9px rgba(102, 74, 216, 0.38),
      inset -3px -4px 7px rgba(42, 12, 112, 0.32);
  }

  .siri-preview::after {
    content: "";
    position: absolute;
    inset: 24%;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.28);
    filter: blur(2px);
  }
</style>
