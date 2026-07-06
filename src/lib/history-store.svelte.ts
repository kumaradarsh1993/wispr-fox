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
    // Auto-titles land a beat AFTER the flow goes idle (parallel LLM call) —
    // Rust emits this when a title (or other out-of-band edit) is written.
    const { listen } = await import("@tauri-apps/api/event");
    const unlistenChanged = await listen("wispr:history_changed", () => {
      void this.refresh();
    });
    this.unsub = () => {
      unlisten();
      unlistenChanged();
    };
  }

  async remove(id: string) {
    await api.deleteRecording(id);
    this.list = this.list.filter((r) => r.id !== id);
  }
}

export const history = new HistoryStore();
