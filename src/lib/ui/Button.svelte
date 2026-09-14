<script lang="ts">
  /**
   * The app's one button. Four tones, two sizes. Pages must not hand-roll
   * button CSS any more (docs/DESIGN_v3.5.md §3).
   *
   *   <Button onclick={save}>Save</Button>
   *   <Button tone="ghost" size="sm" onclick={cancel}>Cancel</Button>
   *   <Button tone="danger" disabled={!armed}>Delete</Button>
   */
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Tone = "primary" | "secondary" | "ghost" | "danger";
  type Size = "sm" | "md";

  let {
    tone = "secondary",
    size = "md",
    type = "button",
    iconOnly = false,
    children,
    ...rest
  }: HTMLButtonAttributes & {
    tone?: Tone;
    size?: Size;
    /** Square hit area; pass `aria-label`. */
    iconOnly?: boolean;
    children: Snippet;
  } = $props();
</script>

<button class="btn" data-tone={tone} data-size={size} data-icon={iconOnly || undefined} {type} {...rest}>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    font-weight: 600;
    line-height: 1;
    white-space: nowrap;
    user-select: none;
    transition:
      background var(--motion-fast) var(--ease-standard),
      border-color var(--motion-fast) var(--ease-standard),
      color var(--motion-fast) var(--ease-standard),
      transform var(--motion-fast) var(--ease-standard);
  }
  .btn:active:not(:disabled) {
    transform: translateY(1px);
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .btn[data-size="md"] {
    height: 34px;
    padding: 0 var(--sp-4);
    font-size: var(--fs-base);
  }
  .btn[data-size="sm"] {
    height: 28px;
    padding: 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .btn[data-icon][data-size="md"] {
    width: 34px;
    padding: 0;
  }
  .btn[data-icon][data-size="sm"] {
    width: 28px;
    padding: 0;
  }

  .btn[data-tone="primary"] {
    color: #fff;
    background: var(--accent);
  }
  .btn[data-tone="primary"]:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn[data-tone="primary"]:active:not(:disabled) {
    background: var(--accent-pressed);
  }

  .btn[data-tone="secondary"] {
    color: var(--text-primary);
    background: var(--bg-card);
    border-color: var(--border);
  }
  .btn[data-tone="secondary"]:hover:not(:disabled) {
    background: var(--bg-elev);
    border-color: var(--text-muted);
  }

  .btn[data-tone="ghost"] {
    color: var(--text-secondary);
    background: transparent;
  }
  .btn[data-tone="ghost"]:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-subtle);
  }

  .btn[data-tone="danger"] {
    color: var(--danger);
    background: transparent;
    border-color: var(--danger-fade);
  }
  .btn[data-tone="danger"]:hover:not(:disabled) {
    color: #fff;
    background: var(--danger);
    border-color: var(--danger);
  }
</style>
