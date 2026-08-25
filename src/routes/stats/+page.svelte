<script lang="ts">
  import { onMount } from "svelte";
  import { statsStore } from "$lib/stats-store.svelte";
  import { TYPING_WPM, fmtDuration, fmtDurationLong, fmtNum, type DayPoint } from "$lib/stats";

  // Chart window + which metric the bars show.
  let windowDays = $state<7 | 30 | 90>(30);
  type Metric = "words" | "time" | "sessions";
  let metric = $state<Metric>("words");

  onMount(() => {
    statsStore.subscribe();
  });

  let d = $derived(statsStore.derived(windowDays));
  let voice = $derived(statsStore.voice);

  function metricVal(p: DayPoint): number {
    return metric === "words" ? p.words : metric === "time" ? p.dictation_ms : p.sessions;
  }
  let series = $derived(d?.series ?? []);
  let maxVal = $derived(Math.max(1, ...series.map(metricVal)));

  function barLabel(p: DayPoint): string {
    const v = metricVal(p);
    if (metric === "time") return fmtDuration(v);
    if (metric === "words") return `${fmtNum(v)} words`;
    return `${v} session${v === 1 ? "" : "s"}`;
  }
  function dayTip(p: DayPoint): string {
    const dt = new Date(p.date + "T00:00:00");
    const nice = dt.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" });
    return `${nice} · ${barLabel(p)}`;
  }
  function shortDay(date: string): string {
    return new Date(date + "T00:00:00").toLocaleDateString([], { day: "numeric" });
  }

  function sinceLabel(firstDay: string | null): string {
    if (!firstDay) return "";
    const dt = new Date(firstDay + "T00:00:00");
    return dt.toLocaleDateString([], { month: "long", day: "numeric", year: "numeric" });
  }

  // Only label every Nth bar on the x-axis so it doesn't crowd at 30/90 days.
  let labelEvery = $derived(windowDays <= 7 ? 1 : windowDays <= 30 ? 3 : 10);
</script>

<div class="stats-page">
  <header class="stats-header">
    <div>
      <p class="wf-kicker">A view from the hill</p>
      <h1 class="wf-page-title">Your voice, in motion</h1>
      {#if d?.firstDay}
        <p class="subtle">Since {sinceLabel(d.firstDay)} · these totals are kept forever, even after recordings are cleared.</p>
      {:else}
        <p class="subtle">Time saved, words, and streaks — kept forever, even after recordings are cleared.</p>
      {/if}
    </div>
  </header>

  {#if !d || d.totalSessions === 0}
    <div class="empty">
      <img class="empty-fox" src="/fox/fox-empty-state.png" alt="" />
      <h2>No dictations yet</h2>
      <p>Press your dictation hotkey and start talking. Your time-saved counter starts the moment you do.</p>
    </div>
  {:else}
    <!-- Hero: time saved -->
    <section class="hero">
      <div class="hero-main">
        <div class="hero-label">Time saved vs typing</div>
        <div class="hero-value">{fmtDurationLong(d.timeSavedMs)}</div>
        <div class="hero-sub">
          You delivered {fmtNum(d.totalWords)} words in {fmtDuration(d.totalDictationMs)} —
          typing them at {TYPING_WPM} wpm would have taken {fmtDuration(d.typingMs)}.
        </div>
      </div>
      <div class="hero-fox" aria-hidden="true">
        <img src="/fox/fox-success.png" alt="" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")} />
      </div>
    </section>

    <!-- Key stat cards -->
    <section class="cards">
      <div class="card">
        <div class="card-key">Words delivered</div>
        <div class="card-val">{fmtNum(d.totalWords)}</div>
        <div class="card-sub">{fmtNum(d.avgWordsPerSession)} avg / session</div>
      </div>
      <div class="card">
        <div class="card-key">Sessions</div>
        <div class="card-val">{fmtNum(d.totalSessions)}</div>
        <div class="card-sub">over {d.activeDays} active day{d.activeDays === 1 ? "" : "s"}</div>
      </div>
      <div class="card">
        <div class="card-key">Active days</div>
        <div class="card-val">{fmtNum(d.activeDays)}</div>
        <div class="card-sub">since {sinceLabel(d.firstDay)}</div>
      </div>
      <div class="card">
        <div class="card-key">Current streak</div>
        <div class="card-val">{d.currentStreak}<span class="unit"> day{d.currentStreak === 1 ? "" : "s"}</span></div>
        <div class="card-sub">{d.today.sessions > 0 ? "today's in the books ✓" : "dictate today to keep it"}</div>
      </div>
    </section>

    {#if voice}
      <section class="voice-block">
        <div class="voice-heading">
          <div>
            <p class="wf-kicker">Your verbal fingerprint</p>
            <h2>Voice signature</h2>
            <p>
              Based on {voice.sessions} retained microphone session{voice.sessions === 1 ? "" : "s"}
              and {fmtNum(voice.words)} raw words. Uploads, meeting speakers, and AI-polished text are excluded.
            </p>
          </div>
          <div class="signature-tags" aria-label="Voice signature summary">
            <span>{voice.paceLabel}</span>
            <span>{voice.sessionStyle}</span>
            <span>{voice.sentenceStyle}</span>
            <span>{voice.vocabularyLabel}</span>
          </div>
        </div>

        <div class="voice-grid">
          <article class="voice-card">
            <div class="voice-card-key">Typical dictation pace</div>
            <div class="voice-card-value">{voice.medianWpm}<span> wpm</span></div>
            <p>Middle half: {voice.paceLow}–{voice.paceHigh} wpm · {voice.paceConsistency.toLowerCase()}</p>
          </article>
          <article class="voice-card">
            <div class="voice-card-key">Thought shape</div>
            <div class="voice-card-value">{fmtNum(voice.medianSessionWords)}<span> words</span></div>
            <p>Typical session · {voice.medianSentenceWords} words per sentence · {voice.questionShare}% questions</p>
          </article>
          <article class="voice-card">
            <div class="voice-card-key">Vocabulary range</div>
            <div class="voice-card-value">{fmtNum(voice.distinctWords)}<span> distinct</span></div>
            <p>{voice.vocabularyLabel} across the retained raw-transcript sample</p>
          </article>
          <article class="voice-card">
            <div class="voice-card-key">Speech texture</div>
            <div class="voice-card-value">{voice.discoursePer100}<span> / 100 words</span></div>
            <p>Captured discourse markers · {voice.repeatedPer100} immediate repeats per 100 words</p>
          </article>
        </div>

        {#if voice.topMarkers.length}
          <div class="marker-row">
            <span class="marker-label">Recurring markers</span>
            {#each voice.topMarkers as marker (marker.label)}
              <span class="marker-chip"><strong>{marker.label}</strong> {marker.count}</span>
            {/each}
          </div>
        {/if}

        <p class="voice-note">
          Calculated on this device from raw transcript text and captured duration. Transcription can omit soft fillers,
          so these are observable patterns rather than accent, pronunciation, or fluency scores.
        </p>
      </section>
    {/if}

    <!-- Chart -->
    <section class="chart-block">
      <div class="chart-toolbar">
        <div class="seg">
          {#each [["words","Words"],["time","Time"],["sessions","Sessions"]] as [val, label] (val)}
            <button class="seg-btn" class:active={metric === val} onclick={() => (metric = val as Metric)}>{label}</button>
          {/each}
        </div>
        <div class="seg">
          {#each [[7,"7d"],[30,"30d"],[90,"90d"]] as [val, label] (val)}
            <button class="seg-btn" class:active={windowDays === val} onclick={() => (windowDays = val as 7 | 30 | 90)}>{label}</button>
          {/each}
        </div>
      </div>

      <div class="chart" role="img" aria-label="Daily dictation activity">
        {#each series as p (p.date)}
          {@const v = metricVal(p)}
          <div class="bar-col" title={dayTip(p)}>
            <div class="bar-wrap">
              <div class="bar" class:zero={v === 0} style="height: {Math.max(2, (v / maxVal) * 100)}%"></div>
            </div>
          </div>
        {/each}
      </div>
      <div class="chart-axis">
        {#each series as p, i (p.date)}
          <div class="axis-tick">{i % labelEvery === 0 ? shortDay(p.date) : ""}</div>
        {/each}
      </div>
    </section>

    <!-- Footnotes / secondary -->
    <section class="footnote">
      {#if d.bestDay}
        <span class="best-day"><i aria-hidden="true"></i> Best day: <strong>{new Date(d.bestDay.date + "T00:00:00").toLocaleDateString([], { month: "short", day: "numeric" })}</strong> with {fmtNum(d.bestDay.words)} words.</span>
      {/if}
      <span>“Time saved” assumes typing at {TYPING_WPM} wpm — a conservative baseline, so the real number is likely higher.</span>
    </section>
  {/if}
</div>

<style>
  /* Full-bleed scroll pane, matching History (.rows) and Settings
     (.section-body). This page used to BE its own scroll container while
     also being `max-width: 1040px; margin: 0 auto` — so on a wide window
     the scrollbar was centred with the content and floated in the middle
     of the pane with dead surface either side of it, instead of sitting on
     the right edge where every other screen puts it. The scroller is now
     the full pane; the content inside decides its own width. */
  .stats-page {
    height: 100vh;
    overflow-y: auto;
    padding: 28px clamp(20px, 3.4vw, 44px) 60px;
    box-sizing: border-box;
    background: var(--bg-surface);
    color: var(--text-primary);
    scrollbar-gutter: stable;
    container: stats / inline-size;
  }

  .stats-header {
    padding: 4px 2px 2px;
  }
  .stats-header h1 {
    margin-bottom: 5px;
  }
  .subtle {
    font-size: 12.5px;
    color: var(--text-secondary);
    margin: 0;
    max-width: 620px;
    line-height: 1.45;
  }

  /* Empty state */
  .empty {
    text-align: center;
    padding: 80px 20px;
    color: var(--text-secondary);
  }
  .empty-fox {
    width: 170px;
    height: 170px;
    object-fit: contain;
    filter: drop-shadow(0 7px 14px rgba(92, 56, 22, 0.13));
  }
  .empty h2 { margin: 12px 0 6px; color: var(--text-primary); font-size: 17px; }
  .empty p { margin: 0 auto; max-width: 380px; font-size: 13px; line-height: 1.5; }

  /* Hero */
  .hero {
    margin-top: 22px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 22px 24px;
    border-radius: var(--radius-xl);
    background:
      linear-gradient(110deg, color-mix(in srgb, var(--accent-fade) 74%, var(--bg-card)), color-mix(in srgb, var(--field-fade) 55%, var(--bg-card)));
    border: 1px solid var(--border-subtle);
    box-shadow: var(--shadow-sm);
    overflow: hidden;
  }
  .hero-label {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent);
  }
  .hero-value {
    font-size: 44px;
    font-weight: 720;
    line-height: 1.05;
    margin: 4px 0 8px;
    letter-spacing: -0.02em;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }
  .hero-sub {
    font-size: 12.5px;
    color: var(--text-secondary);
    max-width: 460px;
    line-height: 1.5;
  }
  .hero-fox {
    flex: 0 0 auto;
    width: 110px;
    height: 110px;
  }
  .hero-fox img { width: 100%; height: 100%; object-fit: contain; }

  /* Cards */
  .cards {
    margin-top: 16px;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }
  .card {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: 14px 16px;
    box-shadow: var(--shadow-xs);
  }
  .card-key {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-secondary);
  }
  .card-val {
    font-size: 26px;
    font-weight: 680;
    margin-top: 6px;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }
  .card-val .unit { font-size: 14px; font-weight: 500; color: var(--text-secondary); }
  .card-sub {
    font-size: 11.5px;
    color: var(--text-secondary);
    margin-top: 3px;
  }

  /* Retained raw-transcript language portrait. Lifetime productivity totals
     live above; this block deliberately explains its narrower voice sample. */
  .voice-block {
    margin-top: 22px;
    padding: 20px;
    border: 1px solid color-mix(in srgb, var(--field) 24%, var(--border-subtle));
    border-radius: var(--radius-xl);
    background:
      radial-gradient(circle at 92% 12%, color-mix(in srgb, var(--sun) 13%, transparent), transparent 30%),
      linear-gradient(135deg, color-mix(in srgb, var(--field-fade) 42%, var(--bg-card)), var(--bg-card) 58%);
    box-shadow: var(--shadow-sm);
  }

  .voice-heading {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 22px;
  }

  .voice-heading h2 {
    margin: 2px 0 5px;
    font-size: 22px;
    letter-spacing: -0.025em;
  }

  .voice-heading p:not(.wf-kicker) {
    margin: 0;
    max-width: 610px;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.5;
  }

  .signature-tags {
    max-width: 300px;
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 6px;
  }

  .signature-tags span,
  .marker-chip {
    border: 1px solid color-mix(in srgb, var(--field) 25%, var(--border-subtle));
    background: color-mix(in srgb, var(--bg-card) 80%, var(--field-fade));
    border-radius: 999px;
    color: var(--text-secondary);
    font-size: 10.5px;
    padding: 5px 8px;
    white-space: nowrap;
  }

  .voice-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    margin-top: 18px;
  }

  .voice-card {
    min-width: 0;
    padding: 13px 14px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-card) 91%, transparent);
    box-shadow: var(--shadow-xs);
  }

  .voice-card-key {
    color: var(--text-secondary);
    font-size: 10.5px;
    font-weight: 650;
    letter-spacing: 0.045em;
    text-transform: uppercase;
  }

  .voice-card-value {
    margin-top: 5px;
    color: var(--text-primary);
    font-size: 23px;
    font-weight: 700;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }

  .voice-card-value span {
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 550;
    letter-spacing: 0;
  }

  .voice-card p {
    margin: 4px 0 0;
    color: var(--text-secondary);
    font-size: 10.5px;
    line-height: 1.4;
  }

  .marker-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 14px;
  }

  .marker-label {
    margin-right: 2px;
    color: var(--text-secondary);
    font-size: 10.5px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .marker-chip strong {
    color: var(--text-primary);
    font-weight: 650;
  }

  .voice-note {
    margin: 13px 0 0;
    color: var(--text-secondary);
    font-size: 10.5px;
    line-height: 1.45;
  }

  /* Chart */
  .chart-block {
    margin-top: 22px;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: 16px 18px 14px;
  }
  .chart-toolbar {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 14px;
    flex-wrap: wrap;
  }
  .seg {
    display: inline-flex;
    background: var(--bg-subtle);
    border-radius: 9px;
    padding: 2px;
    gap: 2px;
  }
  .seg-btn {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 550;
    padding: 4px 12px;
    border-radius: 7px;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease, box-shadow 120ms ease;
  }
  .seg-btn:hover { color: var(--text-primary); }
  .seg-btn.active {
    background: var(--bg-card);
    color: var(--accent);
    box-shadow: 0 1px 3px rgba(0,0,0,0.08);
  }

  .chart {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 170px;
  }
  .bar-col {
    flex: 1 1 0;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .bar-wrap {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: flex-end;
  }
  .bar {
    width: 100%;
    min-width: 3px;
    background: linear-gradient(180deg, var(--accent), var(--accent-fade));
    border-radius: 4px 4px 2px 2px;
    transition: height 280ms cubic-bezier(0.32, 0.72, 0, 1), opacity 160ms ease;
  }
  .bar:hover { background: var(--accent); }
  .bar.zero {
    background: var(--border-subtle);
    border-radius: 3px;
  }
  .chart-axis {
    display: flex;
    gap: 3px;
    margin-top: 6px;
  }
  .axis-tick {
    flex: 1 1 0;
    text-align: center;
    font-size: 9px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
  }

  .footnote {
    margin-top: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .footnote strong { color: var(--text-primary); }

  .best-day {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }

  .best-day i {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--sun);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--sun) 16%, transparent);
  }

  /* Breakpoints are CONTAINER queries, not media queries. These used to be
     @media, which measures the window — but this page only ever gets the
     window minus the sidebar (272px by default, user-resizable, and
     collapsible to 64px). A 1000px window put a ~700px pane through the
     ">920px" four-across layout and squashed it. Measuring the pane is the
     same thing History (@container) and Settings (@container settings) do. */
  @container stats (max-width: 920px) {
    .cards { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .voice-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .voice-heading { flex-direction: column; gap: 12px; }
    .signature-tags { max-width: none; justify-content: flex-start; }
  }

  @container stats (max-width: 720px) {
    .cards { grid-template-columns: repeat(2, 1fr); }
    .hero-fox { display: none; }
  }

  @container stats (max-width: 560px) {
    .voice-grid { grid-template-columns: 1fr; }
  }
</style>
