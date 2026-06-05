<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { statsStore } from "$lib/stats-store.svelte";
  import { fmtDurationLong, fmtNum } from "$lib/stats";

  // 14-day window drives the inline sparkline; totals come from lifetime.
  onMount(() => {
    statsStore.subscribe();
  });

  let d = $derived(statsStore.derived(14));
  let spark = $derived(d?.series ?? []);
  let sparkMax = $derived(Math.max(1, ...spark.map((p) => p.words)));
</script>

{#if d && d.totalSessions > 0}
  <button class="stats-widget" onclick={() => goto("/stats")} title="Open the full analytics dashboard">
    <div class="sw-hero">
      <div class="sw-hero-label">Time saved</div>
      <div class="sw-hero-val">{fmtDurationLong(d.timeSavedMs)}</div>
    </div>

    <div class="sw-divider"></div>

    <div class="sw-metrics">
      <div class="sw-metric">
        <span class="sw-val">{fmtNum(d.totalWords)}</span>
        <span class="sw-key">words</span>
      </div>
      <div class="sw-metric">
        <span class="sw-val">{fmtNum(d.totalSessions)}</span>
        <span class="sw-key">sessions</span>
      </div>
      <div class="sw-metric">
        <span class="sw-val">{d.currentStreak}🔥</span>
        <span class="sw-key">day streak</span>
      </div>
    </div>

    <div class="sw-spark" aria-hidden="true">
      {#each spark as p (p.date)}
        <span class="sw-bar" class:zero={p.words === 0} style="height: {Math.max(8, (p.words / sparkMax) * 100)}%"></span>
      {/each}
    </div>

    <span class="sw-cta">View details →</span>
  </button>
{/if}

<style>
  .stats-widget {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    margin-bottom: 14px;
    border-radius: 14px;
    background: var(--accent-fade);
    border: 1px solid var(--accent-fade);
    cursor: pointer;
    text-align: left;
    transition: transform 120ms ease, box-shadow 120ms ease;
    color: var(--text-primary);
  }
  .stats-widget:hover {
    transform: translateY(-1px);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.1);
  }

  .sw-hero { flex: 0 0 auto; }
  .sw-hero-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent);
  }
  .sw-hero-val {
    font-size: 24px;
    font-weight: 700;
    line-height: 1.05;
    margin-top: 2px;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }

  .sw-divider {
    width: 1px;
    align-self: stretch;
    background: var(--border-subtle);
    margin: 2px 0;
  }

  .sw-metrics {
    display: flex;
    gap: 20px;
    flex: 0 0 auto;
  }
  .sw-metric { display: flex; flex-direction: column; }
  .sw-val {
    font-size: 16px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
  }
  .sw-key {
    font-size: 10.5px;
    color: var(--text-secondary);
  }

  .sw-spark {
    flex: 1 1 auto;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    gap: 2px;
    height: 34px;
    min-width: 60px;
  }
  .sw-bar {
    width: 5px;
    background: var(--accent);
    border-radius: 2px;
    opacity: 0.85;
  }
  .sw-bar.zero { background: var(--border-subtle); opacity: 1; }

  .sw-cta {
    flex: 0 0 auto;
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    white-space: nowrap;
  }

  /* Narrow windows: hide the sparkline first, then the metrics, so the
     headline + CTA always survive. */
  @media (max-width: 720px) {
    .sw-spark { display: none; }
  }
  @media (max-width: 560px) {
    .sw-metrics { gap: 12px; }
    .sw-metric:nth-child(3) { display: none; }
  }
</style>
