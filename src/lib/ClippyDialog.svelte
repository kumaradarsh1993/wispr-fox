<script lang="ts">
  // Reusable Clippy + speech bubble. Used in onboarding for guided steps,
  // and available for tooltips / first-run hints anywhere in the app.
  // Uses the real Microsoft Clippy sprite from the vendored agent.

  import clippyAgent from "./clippyjs-vendor/clippy-agent.js";

  let {
    message,
    /** Sprite frame to show — defaults to the "writing on notepad" pose */
    pose = "default",
    /** Size of the Clippy bitmap in px */
    size = 80,
    /** Layout direction — bubble to the right (default) or below Clippy */
    direction = "right",
  }: {
    message: string;
    pose?: "default" | "wave" | "thinking" | "writing" | "rest";
    size?: number;
    direction?: "right" | "below";
  } = $props();

  // Hand-picked frames from the Clippy sprite sheet for different poses.
  // Frame size is 124x93; these are top-left offsets within the sprite.
  const POSES = {
    rest: { x: 0, y: 0 },
    default: { x: 992, y: 1953 },   // writing-on-notepad — most charming pose
    writing: { x: 992, y: 1953 },
    wave: { x: 0, y: 93 },            // first frame of Greeting animation
    thinking: { x: 992, y: 1395 },    // a Thinking-style pose
  };
  const FRAME_W = 124;
  const FRAME_H = 93;

  let frame = $derived(POSES[pose] ?? POSES.default);
  let spriteW = $derived(Math.round(size * (FRAME_W / FRAME_H)));
  let spriteH = $derived(size);
  let scale = $derived(size / FRAME_H);
</script>

<div class="clippy-dialog" class:below={direction === "below"}>
  <div class="clippy-sprite-wrap" style="width: {spriteW}px; height: {spriteH}px;">
    <div
      class="clippy-sprite-frame"
      style="
        width: {FRAME_W}px;
        height: {FRAME_H}px;
        background: url('{clippyAgent.image}') -{frame.x}px -{frame.y}px no-repeat;
        transform: scale({scale});
        transform-origin: top left;
      "
    ></div>
  </div>

  <div class="bubble" class:bubble-below={direction === "below"}>
    <div class="bubble-arrow"></div>
    <p>{message}</p>
  </div>
</div>

<style>
  .clippy-dialog {
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .clippy-dialog.below {
    flex-direction: column;
    gap: 14px;
  }

  .clippy-sprite-wrap {
    flex-shrink: 0;
    overflow: hidden;
    line-height: 0;
    filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.15));
  }

  .clippy-sprite-frame {
    image-rendering: -webkit-optimize-contrast;
  }

  .bubble {
    position: relative;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 14px 18px;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
    max-width: 420px;
    color: var(--text-primary);
  }

  .bubble p {
    margin: 0;
    font-size: 14px;
    line-height: 1.5;
  }

  .bubble-arrow {
    position: absolute;
    left: -8px;
    top: 50%;
    transform: translateY(-50%) rotate(45deg);
    width: 14px;
    height: 14px;
    background: var(--bg-card);
    border-left: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .bubble-below .bubble-arrow {
    left: 50%;
    top: -8px;
    transform: translateX(-50%) rotate(45deg);
    border-left: none;
    border-bottom: none;
    border-top: 1px solid var(--border);
    border-right: 1px solid var(--border);
  }
</style>
