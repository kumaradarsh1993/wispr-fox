// The fleet: every device signed into this account, plus the MERGED analytics
// across all of them.
//
// Why this exists: `daily_stats` is per-install, so each machine could only
// ever answer "how much have you dictated ON ME". Two desktops reported two
// different "since" dates and two different time-saved totals, and neither was
// the number the user actually wanted. This store merges every device's
// published rollup into one honest account-wide picture, while keeping the
// per-device split available.
//
// Only counts and dates cross the wire. Audio never leaves the machine that
// recorded it, and this adds no transcript text to what sync already carries.

import { listen } from "@tauri-apps/api/event";
import { api, type FleetDevice, type StatsSummary, type DailyStat } from "./api";

/** Merge N per-device rollups into one account-wide summary.
 *
 *  Exported for testing and because the merge rules are the interesting part:
 *  per-day rows are SUMMED across devices that were both used on the same day,
 *  lifetime totals are summed, and `first_day` is the EARLIEST across devices
 *  (the whole point of the feature — the account started when the first device
 *  did, not when this one did). */
export function mergeSummaries(summaries: StatsSummary[]): StatsSummary | null {
  const present = summaries.filter(Boolean);
  if (present.length === 0) return null;

  const byDate = new Map<string, DailyStat>();
  let total_sessions = 0;
  let total_words = 0;
  let total_dictation_ms = 0;
  let first_day: string | null = null;

  for (const s of present) {
    total_sessions += s.total_sessions ?? 0;
    total_words += s.total_words ?? 0;
    total_dictation_ms += s.total_dictation_ms ?? 0;
    // String compare is correct and cheap for "YYYY-MM-DD".
    if (s.first_day && (first_day === null || s.first_day < first_day)) {
      first_day = s.first_day;
    }
    for (const d of s.days ?? []) {
      const cur = byDate.get(d.date);
      if (cur) {
        cur.sessions += d.sessions;
        cur.words += d.words;
        cur.dictation_ms += d.dictation_ms;
        cur.light_count += d.light_count;
        cur.draft_count += d.draft_count;
      } else {
        byDate.set(d.date, { ...d });
      }
    }
  }

  const days = [...byDate.values()].sort((a, b) => a.date.localeCompare(b.date));
  return { days, total_sessions, total_words, total_dictation_ms, first_day };
}

class FleetStore {
  devices = $state<FleetDevice[]>([]);
  loading = $state(false);
  /** Null until the first successful load. */
  error = $state<string | null>(null);
  private subscribed = false;
  private unsub?: () => void;

  /** The device this app is running on, if the registry has caught up. */
  get thisDevice(): FleetDevice | null {
    return this.devices.find((d) => d.this_device) ?? null;
  }

  /** Devices that have actually published analytics. */
  get reporting(): FleetDevice[] {
    return this.devices.filter((d) => d.stats !== null);
  }

  /** True once there is more than one device on the account — the only case
   *  where an "all devices vs this device" control is worth showing at all. */
  get isMultiDevice(): boolean {
    return this.devices.length > 1;
  }

  /** Account-wide merged analytics, or null when nothing has been published. */
  get merged(): StatsSummary | null {
    return mergeSummaries(
      this.devices.map((d) => d.stats).filter((s): s is StatsSummary => s !== null),
    );
  }

  /** Look up a device by the id stamped on a recording. */
  byId(id: string | null | undefined): FleetDevice | null {
    if (!id) return null;
    return this.devices.find((d) => d.id === id) ?? null;
  }

  async refresh(networkFirst = true) {
    this.loading = true;
    try {
      // Paint from cache immediately, then let the network correct it. On a
      // cold start with no cache this is an empty array, which renders as the
      // normal "no other devices" state rather than a spinner.
      if (this.devices.length === 0) {
        try {
          this.devices = await api.listDevicesCached();
        } catch {
          /* cache read is best-effort */
        }
      }
      if (networkFirst) {
        this.devices = await api.listDevices();
      }
      this.error = null;
    } catch (e) {
      console.warn("fleet.refresh failed", e);
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async setMeta(deviceId: string, icon: string | null, label: string | null) {
    // Optimistic: the round-trip includes a full re-pull, and waiting for it
    // makes clicking an icon feel broken.
    const before = this.devices;
    this.devices = this.devices.map((d) =>
      d.id === deviceId ? { ...d, icon, label } : d,
    );
    try {
      this.devices = await api.setDeviceMeta(deviceId, icon, label);
    } catch (e) {
      console.warn("fleet.setMeta failed", e);
      this.devices = before;
      this.error = String(e);
      throw e;
    }
  }

  async subscribe() {
    if (this.subscribed) return;
    this.subscribed = true;
    await this.refresh();
    // Rust emits this at the end of every sync cycle that touched the fleet.
    //
    // Guarded: three separate places subscribe (History, Insights, Account),
    // and the FIRST one wins. If attaching the listener threw, the rejection
    // escaped as an unhandled promise rejection and the caller's `void
    // fleet.subscribe()` swallowed it silently. The device list still works
    // without the listener — it just stops live-updating — so degrade to
    // that rather than making it look like the whole store failed.
    try {
      this.unsub = await listen("wispr:fleet_changed", () => {
        void this.refresh();
      });
    } catch (e) {
      console.warn("fleet: live updates unavailable; list is still current", e);
    }
  }
}

export const fleet = new FleetStore();
