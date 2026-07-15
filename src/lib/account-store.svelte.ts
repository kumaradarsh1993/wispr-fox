// Runes-backed account/sync store. Single source of truth for whether the
// user is signed in (drives the History "Everywhere" delete option, the
// Settings → Account section, and the onboarding sync step) plus the live
// sync status coming off `wispr:sync_status`.

import { api, onSyncStatus, type AuthStatus, type SyncStatusEvent } from "./api";

class AccountStore {
  status = $state<AuthStatus>({
    configured: false,
    signed_in: false,
    email: null,
    user_id: null,
  });
  sync = $state<SyncStatusEvent>({ state: "signed_out", last_synced_at: null });
  private subscribed = false;
  private unsub?: () => void;

  get signedIn(): boolean {
    return this.status.configured && this.status.signed_in;
  }

  async refresh() {
    try {
      this.status = await api.authStatus();
    } catch (e) {
      console.error("account.refresh failed", e);
    }
  }

  async init() {
    await this.refresh();
    if (!this.subscribed) {
      this.subscribed = true;
      this.unsub = await onSyncStatus((s) => {
        this.sync = s;
      });
    }
  }

  setStatus(s: AuthStatus) {
    this.status = s;
  }

  destroy() {
    this.unsub?.();
    this.unsub = undefined;
    this.subscribed = false;
  }
}

export const account = new AccountStore();
