// Daily usage + active models, refreshed on mount + on every flow-state idle
// transition (so it stays current as recordings happen).

import { api, onFlowState, type DailyUsage, type CurrentModels } from "./api";

class UsageStore {
  usage = $state<DailyUsage | null>(null);
  models = $state<CurrentModels | null>(null);
  private subscribed = false;
  private unsub?: () => void;

  async refresh() {
    try {
      const [u, m] = await Promise.all([api.dailyUsage(), api.currentModels()]);
      this.usage = u;
      this.models = m;
    } catch (e) {
      console.warn("usage.refresh failed", e);
    }
  }

  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await this.refresh();
    const unlisten = await onFlowState((s) => {
      if (s === "idle") this.refresh();
    });
    this.unsub = unlisten;
  }
}

export const usageStore = new UsageStore();
