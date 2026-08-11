//! Speaker attribution: the provider-agnostic half of diarization.
//!
//! Deepgram and ElevenLabs return different JSON shapes but the same idea —
//! every word carries a speaker id, and a "turn" is a run of consecutive words
//! sharing one. This module owns the grouping, the label normalisation, and the
//! rendering; the provider clients only have to reshape their own response into
//! `(speaker_id, word, start)` tuples.
//!
//! Kept as a dependency-free leaf module (serde only, no reqwest/tauri) so its
//! tests can be exercised without linking the whole app — `cargo test` can't run
//! locally on Windows for this crate (GNU ld blows the 65k DLL export limit,
//! and the test binary can't resolve its entrypoint), so the pure logic lives
//! somewhere it can be verified in isolation.

use serde::{Deserialize, Serialize};

/// One contiguous run of speech by a single speaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerTurn {
    /// Display label, e.g. "Speaker 1". Providers hand back opaque integer or
    /// string ids; we normalise to a 1-based human label here so the UI, the
    /// summariser, and the stored transcript all agree.
    pub speaker: String,
    pub text: String,
    /// Seconds from the start of the audio, when the provider reports it.
    pub start: Option<f64>,
}

/// Whether a provider id can attribute speech to speakers. Groq and OpenAI
/// both run Whisper, which has no speaker model at all; Deepgram and
/// ElevenLabs Scribe both do it natively. The upload dialog uses this to grey
/// out the checkbox with a reason rather than accepting it and silently
/// returning an unlabelled wall of text.
pub fn provider_supports_diarization(provider: &str, model: &str) -> bool {
    matches!(provider, "deepgram" | "elevenlabs")
        || (provider == "openai" && model == "gpt-4o-transcribe-diarize")
}

/// Render speaker turns into the transcript text we store and display.
/// Blank line between turns so History stays readable and the LLM summariser
/// sees unambiguous turn boundaries.
pub fn render_turns(turns: &[SpeakerTurn]) -> String {
    turns
        .iter()
        .map(|t| format!("{}: {}", t.speaker, t.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Collapse a word-level `(speaker_id, word, start)` stream into contiguous
/// turns.
pub fn turns_from_words<I>(words: I) -> Vec<SpeakerTurn>
where
    I: IntoIterator<Item = (String, String, Option<f64>)>,
{
    // Providers label speakers with arbitrary ids ("0", "speaker_7", …) and
    // don't promise they start at zero or appear in order. Map first-seen order
    // onto "Speaker 1", "Speaker 2", … so the labels the user reads are stable
    // and start where a human would expect.
    let mut order: Vec<String> = Vec::new();
    let mut turns: Vec<SpeakerTurn> = Vec::new();

    for (raw_id, word, start) in words {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        let idx = match order.iter().position(|s| *s == raw_id) {
            Some(i) => i,
            None => {
                order.push(raw_id);
                order.len() - 1
            }
        };
        let label = format!("Speaker {}", idx + 1);

        match turns.last_mut() {
            Some(last) if last.speaker == label => {
                last.text.push(' ');
                last.text.push_str(word);
            }
            _ => turns.push(SpeakerTurn { speaker: label, text: word.to_string(), start }),
        }
    }
    turns
}

/// Distinct speaker count across a set of turns.
#[cfg(test)]
pub fn speaker_count(turns: &[SpeakerTurn]) -> usize {
    turns
        .iter()
        .map(|t| t.speaker.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(speaker: &str, word: &str) -> (String, String, Option<f64>) {
        (speaker.to_string(), word.to_string(), None)
    }

    #[test]
    fn groups_consecutive_words_into_turns() {
        let turns = turns_from_words(vec![
            w("0", "hello"),
            w("0", "there"),
            w("1", "hi"),
            w("0", "bye"),
        ]);
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[0].speaker, "Speaker 1");
        assert_eq!(turns[0].text, "hello there");
        assert_eq!(turns[1].speaker, "Speaker 2");
        assert_eq!(turns[1].text, "hi");
        assert_eq!(turns[2].speaker, "Speaker 1", "returning speaker keeps its label");
        assert_eq!(speaker_count(&turns), 2);
    }

    /// Providers don't promise ids start at 0 or arrive in order; the label a
    /// user sees must still start at "Speaker 1".
    #[test]
    fn normalises_arbitrary_speaker_ids_to_first_seen_order() {
        let turns = turns_from_words(vec![w("speaker_7", "a"), w("speaker_3", "b")]);
        assert_eq!(turns[0].speaker, "Speaker 1");
        assert_eq!(turns[1].speaker, "Speaker 2");
    }

    /// ElevenLabs emits "spacing" entries whose text is whitespace; they must
    /// not produce doubled spaces or split a turn in half.
    #[test]
    fn skips_blank_words() {
        let turns = turns_from_words(vec![w("0", "one"), w("0", "   "), w("0", "two")]);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].text, "one two");
    }

    #[test]
    fn empty_input_yields_no_turns() {
        assert!(turns_from_words(Vec::new()).is_empty());
        assert_eq!(speaker_count(&[]), 0);
    }

    #[test]
    fn keeps_the_start_time_of_the_turn_not_the_last_word() {
        let turns = turns_from_words(vec![
            ("0".into(), "one".into(), Some(1.0)),
            ("0".into(), "two".into(), Some(2.0)),
        ]);
        assert_eq!(turns[0].start, Some(1.0), "turn starts when it started");
    }

    #[test]
    fn renders_turns_with_blank_line_separators() {
        let turns = turns_from_words(vec![w("0", "hi"), w("1", "yo")]);
        assert_eq!(render_turns(&turns), "Speaker 1: hi\n\nSpeaker 2: yo");
    }

    #[test]
    fn only_deepgram_and_elevenlabs_diarize() {
        assert!(provider_supports_diarization("deepgram", "nova-3"));
        assert!(provider_supports_diarization("elevenlabs", "scribe_v2"));
        assert!(provider_supports_diarization("openai", "gpt-4o-transcribe-diarize"));
        assert!(!provider_supports_diarization("groq", "whisper-large-v3"));
        assert!(!provider_supports_diarization("openai", "gpt-transcribe"));
        assert!(!provider_supports_diarization("", ""));
    }
}
