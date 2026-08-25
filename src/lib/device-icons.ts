// Device identity icons — the small glyph the user assigns to each machine so
// a history card reads "🏠 Home desktop · 38m ago" instead of a hostname they
// have to decode.
//
// Deliberately a SMALL, opinionated set organised by where a machine lives
// rather than what it is. "Which of my computers was this?" is a question
// about place and role ("the work laptop", "the desktop at home"), not about
// chassis type — and the platform glyph already covers desktop-vs-phone.
//
// Ids are stored in Supabase (`device_meta:<device_id>`, see
// src-tauri/src/sync/fleet.rs) and are therefore a wire format: NEVER rename
// or reuse an id. Adding is safe — an unknown id from a newer client falls
// back to the platform glyph rather than breaking the row.

export interface DeviceIcon {
  id: string;
  glyph: string;
  label: string;
}

export const DEVICE_ICONS: readonly DeviceIcon[] = [
  { id: "home", glyph: "🏠", label: "Home" },
  { id: "office", glyph: "🏢", label: "Office" },
  { id: "work", glyph: "💼", label: "Work" },
  { id: "desk", glyph: "🖥️", label: "Desk" },
  { id: "laptop", glyph: "💻", label: "Laptop" },
  { id: "mobile", glyph: "📱", label: "Mobile" },
  { id: "tablet", glyph: "📓", label: "Tablet" },
  { id: "travel", glyph: "✈️", label: "Travel" },
  { id: "studio", glyph: "🎙️", label: "Studio" },
  { id: "lab", glyph: "🧪", label: "Testing" },
] as const;

/** Fallback glyph by platform, for a device with no icon assigned yet. */
const PLATFORM_GLYPH: Record<string, string> = {
  desktop: "🖥️",
  web: "🌐",
  mobile: "📱",
};

const BY_ID = new Map(DEVICE_ICONS.map((i) => [i.id, i]));

/** Resolve the glyph to show for a device. Never throws, never returns empty:
 *  an unrecognised icon id (written by a newer client) degrades to the
 *  platform glyph, and an unrecognised platform degrades to a neutral dot. */
export function deviceGlyph(icon?: string | null, platform?: string | null): string {
  if (icon) {
    const hit = BY_ID.get(icon);
    if (hit) return hit.glyph;
  }
  return PLATFORM_GLYPH[platform ?? ""] ?? "•";
}

/** Human name for an assigned icon, for tooltips and the picker. */
export function deviceIconLabel(icon?: string | null): string | null {
  return icon ? (BY_ID.get(icon)?.label ?? null) : null;
}

/** What to CALL a device: the user's label wins over the registered hostname,
 *  and a device with neither is honestly "Unnamed device" rather than blank. */
export function deviceDisplayName(d: {
  label?: string | null;
  name?: string | null;
}): string {
  const label = d.label?.trim();
  if (label) return label;
  const name = d.name?.trim();
  if (name) return name;
  return "Unnamed device";
}

/** "2 minutes ago" / "yesterday" / "3 Aug" for a last-seen timestamp.
 *  Returns null for a missing or unparseable value so callers can omit the
 *  line entirely rather than print "Invalid Date". */
export function lastSeenLabel(iso?: string | null): string | null {
  if (!iso) return null;
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return null;
  const secs = Math.floor((Date.now() - t) / 1000);
  if (secs < 0) return "just now"; // clock skew between devices
  if (secs < 90) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days} days ago`;
  return new Date(t).toLocaleDateString([], { day: "numeric", month: "short" });
}
