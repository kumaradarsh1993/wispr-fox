<script lang="ts" generics="T extends string">
  /**
   * Segmented control. Replaces the three different radio-card/tab styles
   * the survey found (version tabs, avatar visibility, theme, noise level).
   *
   *   <Segmented options={[{value:"raw",label:"Raw"},…]} bind:value />
   */
  let {
    options,
    value = $bindable(),
    size = "md",
    ariaLabel = "",
  }: {
    options: { value: T; label: string; disabled?: boolean; title?: string }[];
    value: T;
    size?: "sm" | "md";
    ariaLabel?: string;
  } = $props();
</script>

<div class="seg" role="tablist" aria-label={ariaLabel || undefined} data-size={size}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="tab"
      aria-selected={o.value === value}
      class="opt"
      disabled={o.disabled}
      title={o.title}
      onclick={() => (value = o.value)}
    >
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
  }
  .opt {
    border: 0;
    background: transparent;
    color: var(--text-secondary);
    font-weight: 600;
    border-radius: calc(var(--radius-sm) - 3px);
    white-space: nowrap;
    transition:
      background var(--motion-fast) var(--ease-standard),
      color var(--motion-fast) var(--ease-standard);
  }
  .seg[data-size="md"] .opt {
    height: 28px;
    padding: 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .seg[data-size="sm"] .opt {
    height: 24px;
    padding: 0 var(--sp-2);
    font-size: var(--fs-xs);
  }
  .opt:hover:not(:disabled) {
    color: var(--text-primary);
  }
  .opt[aria-selected="true"] {
    color: var(--text-primary);
    background: var(--bg-card);
    box-shadow: var(--shadow-xs);
  }
  .opt:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
