import type { Recording } from "./api";

export type SpeakerTurn = { speaker: string; text: string; start?: number | null };

export function speakerNames(rec: Recording): Record<string, string> {
  try {
    const value = JSON.parse(rec.speaker_names || "{}");
    return value && typeof value === "object" && !Array.isArray(value) ? value : {};
  } catch {
    return {};
  }
}

export function speakerTurns(rec: Recording): SpeakerTurn[] {
  if (rec.speaker_turns) {
    try {
      const value = JSON.parse(rec.speaker_turns);
      if (value?.version === 1 && Array.isArray(value.turns)) return value.turns;
    } catch {
      // Fall through to the backwards-compatible flattened transcript parser.
    }
  }
  const source = rec.transcript || "";
  const chunks = source.split(/\n\s*\n(?=Speaker\s+\d+\s*:)/i);
  return chunks.flatMap((chunk) => {
    const match = chunk.match(/^\s*(Speaker\s+\d+)\s*:\s*([\s\S]*)$/i);
    return match ? [{ speaker: match[1], text: match[2].trim(), start: null }] : [];
  });
}

export function namedSpeaker(label: string, names: Record<string, string>): string {
  return names[label]?.trim() || label;
}

export function applySpeakerNames(text: string, names: Record<string, string>): string {
  let result = text;
  for (const [label, name] of Object.entries(names)) {
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
