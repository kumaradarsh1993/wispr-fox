// Pure helpers for the analytics dashboard. The Rust side hands us a lifetime
// rollup (one row per active local day); everything user-facing — time saved,
// streaks, gap-filled chart series — is derived here so it stays testable and
// framework-free.

import type { DailyStat, StatsSummary } from "$lib/api";

// Average sustained typing speed (words/minute) for a competent typist. Used
// as the baseline we compare dictation against to estimate "time saved". A
// deliberately conservative number — most people type 35–45 wpm in real prose
// (vs. the ~190 wpm people speak), so this errs toward UNDER-claiming savings.
export const TYPING_WPM = 40;

export interface DayPoint {
  date: string; // YYYY-MM-DD (local)
  sessions: number;
  words: number;
  dictation_ms: number;
}

export interface StatsDerived {
  totalSessions: number;
  totalWords: number;
  totalDictationMs: number;
  /** Estimated wall-clock to have TYPED the same words at TYPING_WPM. */
  typingMs: number;
  /** typingMs − dictationMs, floored at 0. The headline "time saved". */
  timeSavedMs: number;
  activeDays: number;
  firstDay: string | null;
  avgWordsPerSession: number;
  /** Gap-filled series for the chart (ascending, length = windowDays). */
  series: DayPoint[];
  /** Today's row (local), or a zeroed point if nothing yet. */
  today: DayPoint;
  /** Consecutive days up to and including today with ≥1 session. */
  currentStreak: number;
  /** The single most-active day by words, or null if no data. */
  bestDay: DailyStat | null;
}

/** Local YYYY-MM-DD for a Date. */
export function localDateStr(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** The last `n` local dates, ascending, ending today. */
export function lastNDates(n: number): string[] {
  const out: string[] = [];
  const now = new Date();
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(now);
    d.setDate(now.getDate() - i);
    out.push(localDateStr(d));
  }
  return out;
}

const ZERO_POINT = (date: string): DayPoint => ({
  date,
  sessions: 0,
  words: 0,
  dictation_ms: 0,
});

/** Human duration: "2h 13m", "13m", "47s", "0s". */
export function fmtDuration(ms: number): string {
  if (!ms || ms < 0) ms = Math.max(0, ms || 0);
  const totalSec = Math.round(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

/** Compact, friendly duration for the big headline: "2.1 hrs", "37 min". */
export function fmtDurationLong(ms: number): string {
  const totalMin = ms / 60000;
  if (totalMin >= 90) return `${(totalMin / 60).toFixed(1)} hrs`;
  if (totalMin >= 1) return `${Math.round(totalMin)} min`;
  return `${Math.round(ms / 1000)} sec`;
}

/** Thousands-separated integer. */
export function fmtNum(n: number): string {
  return Math.round(n).toLocaleString();
}

export function deriveStats(summary: StatsSummary, windowDays = 30): StatsDerived {
  const byDate = new Map<string, DailyStat>();
  for (const d of summary.days) byDate.set(d.date, d);

  const totalWords = summary.total_words;
  const totalDictationMs = summary.total_dictation_ms;
  const totalSessions = summary.total_sessions;

  const typingMs = (totalWords / TYPING_WPM) * 60000;
  const timeSavedMs = Math.max(0, typingMs - totalDictationMs);
  const avgWordsPerSession = totalSessions > 0 ? totalWords / totalSessions : 0;

  // Gap-filled chart window.
  const series: DayPoint[] = lastNDates(windowDays).map((date) => {
    const row = byDate.get(date);
    return row
      ? { date, sessions: row.sessions, words: row.words, dictation_ms: row.dictation_ms }
      : ZERO_POINT(date);
  });

  const todayStr = localDateStr(new Date());
  const todayRow = byDate.get(todayStr);
  const today: DayPoint = todayRow
    ? { date: todayStr, sessions: todayRow.sessions, words: todayRow.words, dictation_ms: todayRow.dictation_ms }
    : ZERO_POINT(todayStr);

  // Streak: walk backwards from today while each day has ≥1 session.
  let currentStreak = 0;
  {
    const now = new Date();
    for (let i = 0; ; i++) {
      const d = new Date(now);
      d.setDate(now.getDate() - i);
      const row = byDate.get(localDateStr(d));
      if (row && row.sessions > 0) currentStreak++;
      else break;
    }
  }

  let bestDay: DailyStat | null = null;
  for (const d of summary.days) {
    if (!bestDay || d.words > bestDay.words) bestDay = d;
  }

  return {
    totalSessions,
    totalWords,
    totalDictationMs,
    typingMs,
    timeSavedMs,
    activeDays: summary.days.length,
    firstDay: summary.first_day,
    avgWordsPerSession,
    series,
    today,
    currentStreak,
    bestDay,
  };
}
