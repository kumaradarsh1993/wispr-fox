import type { Recording } from "./api";

export type SpeakerTurn = {
  speaker: string;
  text: string;
  start?: number | null;
  end?: number | null;
};

export type PassState = "done" | "failed" | "skipped" | "running";
export type PassName = "speakers" | "accuracy" | "notes";

/** The `speaker_turns` envelope, version 1 or 2 (docs/CONVERSATIONS_CONTRACT.md).
 *  Every field past `turns` is optional; a missing one means absent. */
export type ConversationDoc = {
  version: number;
  turns: SpeakerTurn[];
  engine?: { provider: string; model: string } | null;
  accuracy?: { text: string; provider: string; model: string } | null;
  passes?: Partial<Record<PassName, PassState>>;
  errors?: Partial<Record<PassName, string>>;
};

/** Parse the envelope. Accepts version 1 or 2; anything else reads as null. */
export function conversationDoc(rec: Recording): ConversationDoc | null {
  if (!rec.speaker_turns) return null;
  try {
    const value = JSON.parse(rec.speaker_turns);
    if (!value || (value.version !== 1 && value.version !== 2)) return null;
    return {
      version: value.version,
      turns: Array.isArray(value.turns) ? value.turns : [],
      engine: value.engine ?? null,
      accuracy: value.accuracy ?? null,
      passes: value.passes && typeof value.passes === "object" ? value.passes : {},
      errors: value.errors && typeof value.errors === "object" ? value.errors : {},
    };
  } catch {
    return null;
  }
}

export function speakerNames(rec: Recording): Record<string, string> {
  try {
    const value = JSON.parse(rec.speaker_names || "{}");
    return value && typeof value === "object" && !Array.isArray(value) ? value : {};
  } catch {
    return {};
  }
}

export function speakerTurns(rec: Recording): SpeakerTurn[] {
  const doc = conversationDoc(rec);
  if (doc && doc.turns.length) return doc.turns;
  const source = rec.transcript || "";
  const chunks = source.split(/\n\s*\n(?=Speaker\s+\d+\s*:)/i);
  return chunks.flatMap((chunk) => {
    const match = chunk.match(/^\s*(Speaker\s+\d+)\s*:\s*([\s\S]*)$/i);
    return match ? [{ speaker: match[1], text: match[2].trim(), start: null }] : [];
  });
}

/** The Whisper (accuracy) transcript, when one was made. */
export function accuracyText(rec: Recording): string | null {
  const text = conversationDoc(rec)?.accuracy?.text?.trim();
  return text ? text : null;
}

export function namedSpeaker(label: string, names: Record<string, string>): string {
  return names[label]?.trim() || label;
}

export function applySpeakerNames(text: string, names: Record<string, string>): string {
  let result = text;
  // Longest label first so "Speaker 12" is replaced before "Speaker 1".
  const entries = Object.entries(names).sort((a, b) => b[0].length - a[0].length);
  for (const [label, name] of entries) {
    if (!name.trim()) continue;
    const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    // Names are presentation metadata: replace stable placeholders wherever
    // they occur without mutating stored transcripts or generated notes.
    result = result.replace(new RegExp(`\\b${escaped}\\b`, "g"), name.trim());
  }
  return result;
}

export function speakerLabels(rec: Recording): string[] {
  return [...new Set(speakerTurns(rec).map((turn) => turn.speaker))];
}

/** `mm:ss`; minutes are not wrapped, so a 75-minute meeting reads 75:12. */
export function mmss(secs: number | null | undefined): string {
  const total = Math.max(0, Math.floor(secs ?? 0));
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
}

/** Speaker turns as plain text with names and timestamps, for copying. */
export function speakersAsText(rec: Recording): string {
  const names = speakerNames(rec);
  return speakerTurns(rec)
    .map((t) => {
      const who = namedSpeaker(t.speaker, names);
      return t.start != null ? `[${mmss(t.start)}] ${who}: ${t.text}` : `${who}: ${t.text}`;
    })
    .join("\n\n");
}
