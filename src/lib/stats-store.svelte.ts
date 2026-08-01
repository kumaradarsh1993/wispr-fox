// Lifetime analytics store. Loads the Rust `stats_summary` rollup and exposes
// both the raw summary and the derived dashboard metrics. Refreshes on mount
// and whenever a flow run returns to idle (i.e. just after a recording lands),
// so the home widget and the Stats page update live as you dictate.

import { listen } from "@tauri-apps/api/event";
import { api, onFlowState, type StatsSummary } from "./api";
import { deriveStats, type StatsDerived } from "./stats";
import { deriveVoiceInsights, type VoiceInsights } from "./voice-insights";

class StatsStore {
  summary = $state<StatsSummary | null>(null);
  voice = $state<VoiceInsights | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  private subscribed = false;
  private unsub?: () => void;
  private unsubHistory?: () => void;

  /** Derived dashboard metrics, or null until the first load completes. */
  derived(windowDays = 30): StatsDerived | null {
    return this.summary ? deriveStats(this.summary, windowDays) : null;
  }

  async refresh() {
    this.loading = true;
    try {
      const [summary, recordings] = await Promise.all([
        api.statsSummary(),
        api.listHistory(1_000),
      ]);
      this.summary = summary;
      this.voice = deriveVoiceInsights(recordings);
      this.error = null;
    } catch (e) {
      console.warn("stats.refresh failed", e);
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await this.refresh();
    this.unsub = await onFlowState((s) => {
      if (s === "idle") this.refresh();
    });
    this.unsubHistory = await listen("wispr:history_changed", () => this.refresh());
  }
}

export const statsStore = new StatsStore();
