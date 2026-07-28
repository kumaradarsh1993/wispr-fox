// Runes-backed account/sync store. Single source of truth for whether the
// user is signed in (drives the History "Everywhere" delete option, the
// Settings → Account section, and the onboarding sync step) plus the live
// sync status coming off `wispr:sync_status`.

import {
  api,
  onAuthStatus,
  onSyncStatus,
  type AuthStatus,
  type SyncStatusEvent,
} from "./api";

class AccountStore {
  status = $state<AuthStatus>({
    configured: false,
    signed_in: false,
    // Assume a restore may be in flight until the first `auth_status` read
    // comes back. Starting at `false` meant the very first paint of the
    // account UI said "Not signed in" even on a machine with a perfectly good
    // stored session.
    restoring: true,
    email: null,
    user_id: null,
  });
  sync = $state<SyncStatusEvent>({ state: "signed_out", last_synced_at: null });
  private subscribed = false;
  private unsubs: Array<() => void> = [];

  get signedIn(): boolean {
    return this.status.configured && this.status.signed_in;
  }

  /** True only when we positively know there is no session — i.e. not while
   *  the launch-time restore is still resolving. Anything that renders a
   *  "signed out" affordance should gate on this, not on `!signedIn`. */
  get signedOut(): boolean {
    return this.status.configured && !this.status.signed_in && !this.status.restoring;
  }

  async refresh() {
    try {
      this.status = await api.authStatus();
    } catch (e) {
      console.error("account.refresh failed", e);
      // A failed read is not evidence of being signed out; just stop claiming
      // a restore is pending so the UI doesn't spin forever.
      this.status = { ...this.status, restoring: false };
    }
  }

  async init() {
    // Subscribe BEFORE the first read, so an `auth_status` event that fires
    // while the read is in flight isn't dropped.
    if (!this.subscribed) {
      this.subscribed = true;
      this.unsubs.push(
        await onAuthStatus((s) => {
          this.status = s;
        }),
      );
      this.unsubs.push(
        await onSyncStatus((s) => {
          this.sync = s;
        }),
      );
    }
    await this.refresh();
  }

  setStatus(s: AuthStatus) {
    this.status = s;
  }

  destroy() {
    for (const un of this.unsubs) un();
    this.unsubs = [];
    this.subscribed = false;
  }
}

export const account = new AccountStore();
