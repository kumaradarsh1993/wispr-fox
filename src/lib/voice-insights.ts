import type { Recording } from "$lib/api";

const WORD_RE = /[\p{L}\p{N}]+(?:['’][\p{L}\p{N}]+)*/gu;

const DISCOURSE_MARKERS = [
  ["you know", ["you", "know"]],
  ["I mean", ["i", "mean"]],
  ["sort of", ["sort", "of"]],
  ["kind of", ["kind", "of"]],
  ["basically", ["basically"]],
  ["actually", ["actually"]],
  ["literally", ["literally"]],
  ["right", ["right"]],
  ["okay", ["okay"]],
  ["um", ["um"]],
  ["uh", ["uh"]],
  ["erm", ["erm"]],
  ["hmm", ["hmm"]],
] as const;

export interface VoiceMarker {
  label: string;
  count: number;
}

export interface VoiceInsights {
  sessions: number;
  words: number;
  oldestSession: string | null;
  medianSessionWords: number;
  medianWpm: number;
  paceLow: number;
  paceHigh: number;
  paceLabel: string;
  paceConsistency: string;
  medianSentenceWords: number;
  sentenceStyle: string;
  distinctWords: number;
  vocabularyBreadth: number;
  vocabularyLabel: string;
  discoursePer100: number;
  repeatedPer100: number;
  questionShare: number;
  topMarkers: VoiceMarker[];
  sessionStyle: string;
}

function wordsIn(text: string): string[] {
  return (text.toLocaleLowerCase().match(WORD_RE) ?? []).filter(Boolean);
}

function median(values: number[]): number {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function quantile(values: number[], q: number): number {
  if (!values.length) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const pos = (sorted.length - 1) * q;
  const base = Math.floor(pos);
  const rest = pos - base;
  return sorted[base + 1] === undefined
    ? sorted[base]
    : sorted[base] + rest * (sorted[base + 1] - sorted[base]);
}

function movingTypeTokenRatio(words: string[], windowSize = 50): number {
  if (!words.length) return 0;
  if (words.length <= windowSize) return new Set(words).size / words.length;

  const ratios: number[] = [];
  for (let start = 0; start + windowSize <= words.length; start += windowSize) {
    ratios.push(new Set(words.slice(start, start + windowSize)).size / windowSize);
  }
  return ratios.reduce((sum, value) => sum + value, 0) / ratios.length;
}

function markerCounts(words: string[]): VoiceMarker[] {
  const counts = new Map<string, number>();
  for (const [label, phrase] of DISCOURSE_MARKERS) {
    let count = 0;
    for (let i = 0; i <= words.length - phrase.length; i++) {
      if (phrase.every((part, offset) => words[i + offset] === part)) count++;
    }
    if (count > 0) counts.set(label, count);
  }
  return [...counts.entries()]
    .map(([label, count]) => ({ label, count }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}

function paceLabel(wpm: number): string {
  if (wpm < 105) return "Measured";
  if (wpm < 145) return "Conversational";
  if (wpm < 185) return "Brisk";
  return "Rapid-flowing";
}

function sessionStyle(words: number): string {
  if (words < 40) return "Concise bursts";
  if (words < 110) return "Focused passages";
  if (words < 250) return "Expansive thoughts";
  return "Long-form thinker";
}

function sentenceStyle(words: number): string {
  if (words <= 8) return "Direct thought units";
  if (words <= 15) return "Balanced sentences";
  return "Layered sentences";
}

function vocabularyLabel(score: number): string {
  if (score < 54) return "Familiar vocabulary";
  if (score < 68) return "Varied vocabulary";
  return "Wide-ranging vocabulary";
}

/**
 * A privacy-preserving, transcript-first portrait of recent speech. Only raw
 * microphone transcripts are considered: uploads can contain other speakers,
 * while cleaned/drafted text would erase the very habits we want to measure.
 * Audio duration supplies pace, but no audio is uploaded or phonetically
 * classified. Consequently these insights describe language and rhythm, not
 * accent, vocal quality, or pronunciation.
 */
export function deriveVoiceInsights(recordings: Recording[]): VoiceInsights | null {
  const samples = recordings
    .filter((recording) =>
      recording.status === "done" &&
      (recording.source || "mic") === "mic" &&
      Boolean(recording.transcript?.trim()),
    )
    .map((recording) => {
      const transcript = recording.transcript!.trim();
      const words = wordsIn(transcript);
      const durationMs = recording.audio_captured_ms ?? recording.duration_ms;
      return { recording, transcript, words, durationMs };
    })
    .filter((sample) => sample.words.length >= 3);

  const allWords = samples.flatMap((sample) => sample.words);
  if (samples.length < 3 || allWords.length < 250) return null;

  const sessionWordCounts = samples.map((sample) => sample.words.length);
  const pace = samples
    .filter((sample) => sample.durationMs >= 3_000)
    .map((sample) => sample.words.length / (sample.durationMs / 60_000))
    .filter((wpm) => Number.isFinite(wpm) && wpm >= 20 && wpm <= 360);
  const typicalWpm = median(pace);
  const paceLow = quantile(pace, 0.25);
  const paceHigh = quantile(pace, 0.75);
  const paceSpread = typicalWpm > 0 ? (paceHigh - paceLow) / typicalWpm : 0;

  const sentences = samples.flatMap((sample) =>
    sample.transcript
      .split(/[.!?]+/u)
      .map((sentence) => wordsIn(sentence).length)
      .filter((length) => length > 0),
  );
  const questionCount = samples.reduce(
    (count, sample) => count + (sample.transcript.match(/\?/g)?.length ?? 0),
    0,
  );
  const repeatedWords = samples.reduce(
    (count, sample) => count + sample.words.reduce(
      (subtotal, word, index) => subtotal + (index > 0 && word === sample.words[index - 1] ? 1 : 0),
      0,
    ),
    0,
  );
  const markers = markerCounts(allWords);
  const markerTotal = markers.reduce((sum, marker) => sum + marker.count, 0);
  const vocabularyBreadth = movingTypeTokenRatio(allWords) * 100;
  const typicalSentence = median(sentences);
  const typicalSession = median(sessionWordCounts);
  const dated = samples
    .map((sample) => sample.recording.created_at)
    .filter(Boolean)
    .sort();

  return {
    sessions: samples.length,
    words: allWords.length,
    oldestSession: dated[0] ?? null,
    medianSessionWords: Math.round(typicalSession),
    medianWpm: Math.round(typicalWpm),
    paceLow: Math.round(paceLow),
    paceHigh: Math.round(paceHigh),
    paceLabel: paceLabel(typicalWpm),
    paceConsistency: paceSpread < 0.18 ? "Very steady" : paceSpread < 0.35 ? "Naturally flexible" : "Highly dynamic",
    medianSentenceWords: Math.round(typicalSentence),
    sentenceStyle: sentenceStyle(typicalSentence),
    distinctWords: new Set(allWords).size,
    vocabularyBreadth: Math.round(vocabularyBreadth),
    vocabularyLabel: vocabularyLabel(vocabularyBreadth),
    discoursePer100: Math.round((markerTotal / allWords.length) * 1_000) / 10,
    repeatedPer100: Math.round((repeatedWords / allWords.length) * 1_000) / 10,
    questionShare: Math.round((questionCount / Math.max(1, sentences.length)) * 100),
    topMarkers: markers.slice(0, 5),
    sessionStyle: sessionStyle(typicalSession),
  };
}
