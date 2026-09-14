<script lang="ts">
  /**
   * A labelled setting row: label, optional help, the control, and the
   * default value rendered next to it so users can reason about what they
   * changed ("Default: On"). Survey finding: no page showed defaults.
   *
   *   <Field label="Launch at login" help="Starts in the tray." default="Off">
   *     <input type="checkbox" bind:checked={…} />
   *   </Field>
   */
  import type { Snippet } from "svelte";

  let {
    label,
    help = "",
    default: def = "",
    inline = true,
    children,
  }: {
    label: string;
    help?: string;
    /** Human-readable default, e.g. "On", "F8", "7 days". Empty hides it. */
    default?: string;
    /** Control sits on the same line (toggles/selects) vs below (text, sliders). */
    inline?: boolean;
    children: Snippet;
  } = $props();
</script>

<div class="field" data-inline={inline || undefined}>
  <div class="text">
    <div class="label">{label}</div>
    {#if help}<div class="help">{help}</div>{/if}
    {#if def}<div class="default">Default: {def}</div>{/if}
  </div>
  <div class="control">
    {@render children()}
  </div>
</div>

<style>
  .field {
    display: grid;
    gap: var(--sp-2);
    padding: var(--sp-3) 0;
  }
  .field[data-inline] {
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: var(--sp-4);
  }
  .label {
    color: var(--text-primary);
    font-size: var(--fs-base);
    font-weight: 600;
    line-height: var(--lh-tight);
  }
  .help {
    margin-top: 3px;
    color: var(--text-secondary);
    font-size: var(--fs-sm);
    line-height: var(--lh-body);
    max-width: 52ch;
  }
  .default {
    margin-top: 3px;
    color: var(--text-muted);
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }
  .control {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-2);
    min-width: 0;
  }
  .field:not([data-inline]) .control {
    justify-content: flex-start;
  }
</style>
