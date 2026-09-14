<script lang="ts">
  /**
   * The one progressive-disclosure pattern. Settings pages put their
   * Advanced tier in exactly one of these at the bottom; history cards use
   * it for Details. Replaces the ad-hoc <details> blocks the survey found.
   *
   *   <Disclosure label="Advanced">…</Disclosure>
   *   <Disclosure label="Details" hint="timings, engines, file" bind:open />
   */
  import type { Snippet } from "svelte";

  let {
    label,
    hint = "",
    open = $bindable(false),
    children,
  }: { label: string; hint?: string; open?: boolean; children: Snippet } = $props();

  const id = `disc-${Math.random().toString(36).slice(2, 8)}`;
</script>

<div class="disc" data-open={open || undefined}>
  <button
    class="head"
    type="button"
    aria-expanded={open}
    aria-controls={id}
    onclick={() => (open = !open)}
  >
    <svg class="chev" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
      <path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </button>
  {#if open}
    <div class="body" {id}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .disc {
    border-top: 1px solid var(--border-subtle);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    padding: var(--sp-3) 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font-size: var(--fs-sm);
    font-weight: 600;
    text-align: left;
    border-radius: var(--radius-sm);
  }
  .head:hover {
    color: var(--text-primary);
  }
  .chev {
    flex: none;
    transition: transform var(--motion-fast) var(--ease-standard);
  }
  .disc[data-open] .chev {
    transform: rotate(90deg);
  }
  .hint {
    margin-left: auto;
    color: var(--text-muted);
    font-weight: 500;
  }
  .body {
    padding: 0 0 var(--sp-4);
    display: grid;
    gap: var(--sp-4);
  }
</style>
