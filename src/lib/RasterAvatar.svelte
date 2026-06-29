<script lang="ts">
  import {
    RASTER_AVATAR_STATES,
    rasterAvatarForSkin,
    type AvatarState,
  } from "./avatar-packs";
  import type { Skin } from "./skin-store.svelte";

  type LiveState = "idle" | "listening" | "thinking" | "writing" | "pasting";

  let {
    skin,
    state,
    mode,
    hovering,
    phewActive,
  }: {
    skin: Skin;
    state: LiveState;
    mode: string;
    hovering: boolean;
    phewActive: boolean;
  } = $props();

  let pack = $derived(rasterAvatarForSkin(skin));
  let activeState = $derived((pack?.states[state as AvatarState] ? state : "idle") as AvatarState);
</script>

{#if pack}
  <div
    class={`character raster-avatar avatar-${pack.id}-skin`}
    class:hovering={hovering}
    class:phew={phewActive}
    style={`--avatar-w:${pack.art.w}px; --avatar-h:${pack.art.h}px;`}
    data-state={state}
    data-mode={mode}
    data-skin={pack.id}
    aria-hidden="true"
  >
    <div class="raster-aura"></div>

    {#each RASTER_AVATAR_STATES as s (s)}
      <img
        class="raster-frame"
        class:active={s === activeState}
        src={pack.states[s]}
        alt=""
        draggable="false"
        loading="eager"
        decoding="async"
      />
    {/each}

    {#if state === "listening"}
      <div class="raster-signal signal-left"><span></span><span></span><span></span></div>
      <div class="raster-signal signal-right"><span></span><span></span><span></span></div>
    {:else if state === "thinking"}
      <div class="raster-orbit"><span></span><span></span><span></span></div>
    {:else if state === "writing"}
      <div class="raster-pen-glint"><span></span><span></span></div>
    {:else if state === "pasting"}
      <div class="raster-confetti"><span></span><span></span><span></span><span></span></div>
    {/if}

    {#if phewActive}
      <div class="raster-phew"></div>
    {/if}
  </div>
{/if}

<style>
  .raster-avatar {
    position: relative;
    width: var(--avatar-w);
    height: var(--avatar-h);
    transform-origin: bottom center;
    animation: raster-breathe 4.8s ease-in-out infinite;
    pointer-events: auto;
  }

  .raster-frame {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    object-position: center bottom;
    opacity: 0;
    user-select: none;
    -webkit-user-drag: none;
    transition:
      opacity 180ms ease,
      transform 220ms ease,
      filter 220ms ease;
    filter: drop-shadow(0 9px 10px rgba(16, 18, 24, 0.24));
    transform: translateY(0) scale(1);
    z-index: 2;
  }

  .raster-frame.active {
    opacity: 1;
  }

  .raster-aura {
    position: absolute;
    left: 50%;
    bottom: 4%;
    width: 64%;
    height: 22%;
    border-radius: 999px;
    background:
      radial-gradient(ellipse at center, rgba(26, 214, 255, 0.28), rgba(26, 214, 255, 0.1) 44%, transparent 72%);
    transform: translateX(-50%) scaleX(0.92);
    opacity: 0;
    filter: blur(5px);
    z-index: 1;
    transition: opacity 180ms ease, transform 180ms ease;
  }

  .avatar-codex-fox-skin .raster-aura {
    background:
      radial-gradient(ellipse at center, rgba(62, 200, 255, 0.28), rgba(62, 200, 255, 0.1) 45%, transparent 72%);
  }

  .avatar-oru-gujia-skin .raster-aura {
    background:
      radial-gradient(ellipse at center, rgba(236, 124, 52, 0.18), rgba(255, 245, 226, 0.18) 48%, transparent 75%);
  }

  .avatar-spark-buddy-skin .raster-aura {
    background:
      radial-gradient(ellipse at center, rgba(64, 255, 226, 0.42), rgba(255, 219, 58, 0.16) 48%, transparent 75%);
  }

  .raster-avatar[data-state="listening"] {
    animation: raster-listen-bounce 900ms cubic-bezier(0.2, 0.8, 0.28, 1.15) infinite;
  }

  .raster-avatar[data-state="thinking"] {
    animation: raster-think-float 2.5s ease-in-out infinite;
  }

  .raster-avatar[data-state="writing"] {
    animation: raster-writing-focus 1.1s ease-in-out infinite;
  }

  .raster-avatar[data-state="pasting"] {
    animation: raster-pop 720ms cubic-bezier(0.2, 0.9, 0.25, 1.25) both;
  }

  .raster-avatar.hovering .raster-frame.active {
    transform: translateY(-3px) rotate(-1.2deg) scale(1.025);
    filter:
      drop-shadow(0 11px 12px rgba(16, 18, 24, 0.28))
      drop-shadow(0 0 8px rgba(72, 213, 255, 0.18));
  }

  .raster-avatar[data-state="listening"] .raster-aura,
  .raster-avatar[data-state="thinking"] .raster-aura,
  .raster-avatar[data-state="writing"] .raster-aura,
  .raster-avatar[data-state="pasting"] .raster-aura,
  .raster-avatar.hovering .raster-aura {
    opacity: 1;
    transform: translateX(-50%) scaleX(1);
  }

  .raster-signal {
    position: absolute;
    top: 31%;
    width: 28px;
    height: 44px;
    z-index: 3;
    opacity: 0.9;
    pointer-events: none;
  }

  .signal-left { left: -5px; }
  .signal-right { right: -5px; transform: scaleX(-1); }

  .raster-signal span {
    position: absolute;
    top: 50%;
    left: 0;
    border: 2px solid rgba(73, 218, 255, 0.9);
    border-left: 0;
    border-top-color: transparent;
    border-bottom-color: transparent;
    border-radius: 0 999px 999px 0;
    transform: translateY(-50%) scale(0.7);
    opacity: 0;
    filter: drop-shadow(0 0 5px rgba(73, 218, 255, 0.75));
    animation: raster-signal 1.25s ease-out infinite;
  }

  .raster-signal span:nth-child(1) {
    width: 11px;
    height: 24px;
    animation-delay: 0ms;
  }
  .raster-signal span:nth-child(2) {
    width: 18px;
    height: 34px;
    animation-delay: 160ms;
  }
  .raster-signal span:nth-child(3) {
    width: 25px;
    height: 44px;
    animation-delay: 320ms;
  }

  .raster-orbit {
    position: absolute;
    top: 3%;
    left: 50%;
    width: 66px;
    height: 34px;
    transform: translateX(-50%);
    z-index: 3;
    pointer-events: none;
  }

  .raster-orbit span {
    position: absolute;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(97, 215, 255, 0.92);
    box-shadow: 0 0 9px rgba(97, 215, 255, 0.8);
    animation: raster-dot-orbit 1.8s ease-in-out infinite;
  }
  .raster-orbit span:nth-child(1) { left: 8px; top: 21px; animation-delay: 0ms; }
  .raster-orbit span:nth-child(2) { left: 28px; top: 7px; animation-delay: 140ms; }
  .raster-orbit span:nth-child(3) { left: 50px; top: 18px; animation-delay: 280ms; }

  .raster-pen-glint,
  .raster-confetti,
  .raster-phew {
    position: absolute;
    inset: 0;
    z-index: 3;
    pointer-events: none;
  }

  .raster-pen-glint span {
    position: absolute;
    right: 12%;
    bottom: 21%;
    width: 18px;
    height: 2px;
    border-radius: 999px;
    background: rgba(85, 220, 255, 0.92);
    box-shadow: 0 0 8px rgba(85, 220, 255, 0.82);
    transform-origin: left center;
    animation: raster-glint 950ms ease-in-out infinite;
  }
  .raster-pen-glint span:nth-child(2) {
    right: 8%;
    bottom: 29%;
    animation-delay: 180ms;
  }

  .raster-confetti span {
    position: absolute;
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: #57dcff;
    box-shadow: 0 0 8px rgba(87, 220, 255, 0.75);
    animation: raster-confetti-pop 860ms ease-out both;
  }
  .raster-confetti span:nth-child(1) { left: 16%; top: 24%; background: #57dcff; }
  .raster-confetti span:nth-child(2) { right: 15%; top: 20%; background: #ffd34d; animation-delay: 80ms; }
  .raster-confetti span:nth-child(3) { left: 28%; top: 13%; background: #ffffff; animation-delay: 150ms; }
  .raster-confetti span:nth-child(4) { right: 27%; top: 10%; background: #58ffe6; animation-delay: 220ms; }

  .raster-phew::before {
    content: "";
    position: absolute;
    right: 21%;
    top: 22%;
    width: 8px;
    height: 14px;
    border-radius: 999px 999px 999px 0;
    background: linear-gradient(180deg, #9ee8ff, #2ca7ec);
    box-shadow: 0 0 7px rgba(92, 205, 255, 0.7);
    transform: rotate(28deg);
    animation: raster-phew-drop 700ms ease-out both;
  }

  @keyframes raster-breathe {
    0%, 100% { transform: translateY(0) scale(1); }
    50% { transform: translateY(-2px) scale(1.01); }
  }

  @keyframes raster-listen-bounce {
    0%, 100% { transform: translateY(0) scaleX(1) scaleY(1); }
    45% { transform: translateY(-7px) scaleX(0.985) scaleY(1.025); }
    70% { transform: translateY(1px) scaleX(1.018) scaleY(0.988); }
  }

  @keyframes raster-think-float {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50% { transform: translateY(-5px) rotate(1deg); }
  }

  @keyframes raster-writing-focus {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    40% { transform: translateY(-1px) rotate(-0.7deg); }
    70% { transform: translateY(0) rotate(0.7deg); }
  }

  @keyframes raster-pop {
    0% { transform: translateY(0) scale(0.92); }
    55% { transform: translateY(-9px) scale(1.08); }
    100% { transform: translateY(0) scale(1); }
  }

  @keyframes raster-signal {
    0% { opacity: 0; transform: translateY(-50%) scale(0.66); }
    20% { opacity: 1; }
    100% { opacity: 0; transform: translateY(-50%) scale(1.12); }
  }

  @keyframes raster-dot-orbit {
    0%, 100% { transform: translateY(0) scale(0.88); opacity: 0.55; }
    50% { transform: translateY(-8px) scale(1.18); opacity: 1; }
  }

  @keyframes raster-glint {
    0%, 100% { opacity: 0; transform: translateX(0) rotate(-18deg) scaleX(0.45); }
    45% { opacity: 1; transform: translateX(-16px) rotate(-18deg) scaleX(1); }
  }

  @keyframes raster-confetti-pop {
    0% { opacity: 0; transform: translateY(16px) scale(0.4) rotate(0deg); }
    25% { opacity: 1; }
    100% { opacity: 0; transform: translateY(-28px) scale(1.05) rotate(170deg); }
  }

  @keyframes raster-phew-drop {
    0% { opacity: 0; transform: translateY(-8px) rotate(28deg) scale(0.6); }
    25% { opacity: 1; }
    100% { opacity: 0; transform: translateY(12px) rotate(28deg) scale(1); }
  }
</style>
