<script lang="ts">
  // Live input meter, in real dBFS against a target band.
  //
  // Deliberately NOT a bare bouncing bar. A bar looks "fine" at -46 dBFS —
  // it moves, so the mic is obviously working — and that is exactly the level
  // at which a transcript comes back with words silently deleted. A number
  // against a labelled target makes "too quiet" legible instead of leaving the
  // user to infer it from a wiggle.
  //
  // Bands: below -40 too quiet · -40..-25 low · -25..-12 good · above -3 clipping.

  let { rmsDb = -120, peakDb = -120 } = $props<{ rmsDb?: number; peakDb?: number }>();

  const FLOOR = -60;
  const CEIL = 0;

  /** Map dBFS onto 0-100% of the meter's width. */
  function pct(db: number): number {
    if (!Number.isFinite(db)) return 0;
    return Math.max(0, Math.min(100, ((db - FLOOR) / (CEIL - FLOOR)) * 100));
  }

  let rmsPct = $derived(pct(rmsDb));
  let peakPct = $derived(pct(peakDb));

  type Verdict = { kind: "silent" | "quiet" | "low" | "good" | "hot"; text: string };
  let verdict = $derived.by<Verdict>(() => {
    if (rmsDb <= -70) return { kind: "silent", text: "No signal — say something" };
    if (peakDb > -3) return { kind: "hot", text: "Too loud — risk of clipping" };
    if (rmsDb < -40)
      return { kind: "quiet", text: "Too quiet — words will go missing" };
    if (rmsDb < -25) return { kind: "low", text: "A bit quiet — move closer or raise gain" };
    if (rmsDb <= -12) return { kind: "good", text: "Good level" };
    return { kind: "hot", text: "Loud — back off slightly" };
  });

  function fmt(db: number): string {
    return Number.isFinite(db) && db > -100 ? `${db.toFixed(1)} dBFS` : "—";
  }
</script>

<div class="meter-wrap">
  <div class="track" role="img" aria-label={`Input level ${fmt(rmsDb)}, ${verdict.text}`}>
    <!-- Target band (-25 to -12 dBFS) painted into the track so "aim here" is
         visible without reading the numbers. -->
    <div
      class="target-band"
      style={`left: ${pct(-25)}%; width: ${pct(-12) - pct(-25)}%;`}
    ></div>
    <div class="fill" class:quiet={verdict.kind === "quiet" || verdict.kind === "silent"} class:low={verdict.kind === "low"} class:good={verdict.kind === "good"} class:hot={verdict.kind === "hot"} style={`width: ${rmsPct}%`}></div>
    <div class="peak" style={`left: ${peakPct}%`}></div>
  </div>

  <div class="scale" aria-hidden="true">
    <span style={`left: ${pct(-60)}%`}>-60</span>
    <span style={`left: ${pct(-40)}%`}>-40</span>
    <span style={`left: ${pct(-25)}%`}>-25</span>
    <span style={`left: ${pct(-12)}%`}>-12</span>
    <span style={`left: ${pct(0)}%`}>0</span>
  </div>

  <div class="readout">
    <span class="verdict" class:quiet={verdict.kind === "quiet" || verdict.kind === "silent"} class:low={verdict.kind === "low"} class:good={verdict.kind === "good"} class:hot={verdict.kind === "hot"}>
      {verdict.text}
    </span>
    <span class="numbers">avg {fmt(rmsDb)} · peak {fmt(peakDb)}</span>
  </div>
</div>

<style>
  .meter-wrap {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .track {
    position: relative;
    height: 18px;
    border-radius: 9px;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .target-band {
    position: absolute;
    top: 0;
    bottom: 0;
    background: rgba(46, 158, 91, 0.16);
    border-left: 1px dashed rgba(46, 158, 91, 0.5);
    border-right: 1px dashed rgba(46, 158, 91, 0.5);
  }
  .fill {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    border-radius: 9px 0 0 9px;
    transition: width 70ms linear;
  }
  .fill.quiet {
    background: var(--danger);
  }
  .fill.low {
    background: #d98b2b;
  }
  .fill.good {
    background: #2e9e5b;
  }
  .fill.hot {
    background: var(--danger);
  }
  .peak {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--text-primary);
    opacity: 0.55;
    transition: left 70ms linear;
  }

  .scale {
    position: relative;
    height: 12px;
    font-size: 9px;
    color: var(--text-secondary);
  }
  .scale span {
    position: absolute;
    transform: translateX(-50%);
    white-space: nowrap;
  }

  .readout {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
    font-size: 12px;
    flex-wrap: wrap;
  }
  .verdict {
    font-weight: 600;
  }
  .verdict.quiet,
  .verdict.hot {
    color: var(--danger);
  }
  .verdict.low {
    color: #d98b2b;
  }
  .verdict.good {
    color: #2e9e5b;
  }
  .numbers {
    color: var(--text-secondary);
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 11px;
  }
</style>
