// Runes-backed history store. Listens to `wispr:state` events to refresh
// after a recording completes — saves the user from manually pulling.

import { api, onFlowState, type Recording } from "./api";

class HistoryStore {
  list = $state<Recording[]>([]);
  loading = $state(false);
  private subscribed = false;
  private unsub?: () => void;

  async refresh(limit = 100) {
    this.loading = true;
    try {
      this.list = await api.listHistory(limit);
    } catch (e) {
      console.error("history.refresh failed", e);
    } finally {
      this.loading = false;
    }
  }

  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    const unlisten = await onFlowState((s) => {
      // Refresh on terminal states only, to avoid query thrash mid-flow.
      if (s === "idle") this.refresh();
    });
    this.unsub = unlisten;
  }

  async remove(id: string) {
    await api.deleteRecording(id);
    this.list = this.list.filter((r) => r.id !== id);
  }
}

export const history = new HistoryStore();
