//! Conversations: the `speaker_turns` v2 envelope, per-pass bookkeeping, and
//! the Markdown export. Authoritative contract: `docs/CONVERSATIONS_CONTRACT.md`.
//!
//! Pure logic only (serde + std), so every rule here is unit-tested without
//! touching the network, the database or Tauri. `flow.rs` runs the passes and
//! feeds their outcomes in; the UI and the export read the result back out.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::stt::SpeakerTurn;

/// Version this build writes. Readers accept 1 and 2.
pub const WRITE_VERSION: u32 = 2;

/// The three independent passes of a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Speakers,
    Accuracy,
    Notes,
}

impl Pass {
    pub fn key(self) -> &'static str {
        match self {
            Pass::Speakers => "speakers",
            Pass::Accuracy => "accuracy",
            Pass::Notes => "notes",
        }
    }

    pub fn parse(s: &str) -> Option<Pass> {
        match s {
            "speakers" => Some(Pass::Speakers),
            "accuracy" => Some(Pass::Accuracy),
            "notes" => Some(Pass::Notes),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PassState {
    Done,
    Failed,
    Skipped,
    Running,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Passes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speakers: Option<PassState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<PassState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<PassState>,
}

impl Passes {
    fn slot(&mut self, pass: Pass) -> &mut Option<PassState> {
        match pass {
            Pass::Speakers => &mut self.speakers,
            Pass::Accuracy => &mut self.accuracy,
            Pass::Notes => &mut self.notes,
        }
    }

    pub fn get(&self, pass: Pass) -> Option<PassState> {
        match pass {
            Pass::Speakers => self.speakers,
            Pass::Accuracy => self.accuracy,
            Pass::Notes => self.notes,
        }
    }

    fn is_empty(&self) -> bool {
        self.speakers.is_none() && self.accuracy.is_none() && self.notes.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Accuracy {
    pub text: String,
    pub provider: String,
    pub model: String,
}

/// The `speaker_turns` column. Unknown keys are carried through untouched so a
/// newer client's additions survive a round trip through this one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnsDoc {
    pub version: u32,
    #[serde(default)]
    pub turns: Vec<SpeakerTurn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<Accuracy>,
    #[serde(default, skip_serializing_if = "Passes::is_empty")]
    pub passes: Passes,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub errors: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for TurnsDoc {
    fn default() -> Self {
        Self {
            version: WRITE_VERSION,
            turns: Vec::new(),
            engine: None,
            accuracy: None,
            passes: Passes::default(),
            errors: BTreeMap::new(),
            extra: serde_json::Map::new(),
        }
    }
}

impl TurnsDoc {
    /// Read a stored envelope. Accepts version 1 or 2 and treats every
    /// missing field as absent. Anything else (garbage, a future version
    /// with a different shape) reads as `None`.
    pub fn parse(json: &str) -> Option<TurnsDoc> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let version = value.get("version")?.as_u64()?;
        if version != 1 && version != 2 {
            return None;
        }
        serde_json::from_value(value).ok()
    }

    /// Parse, or start a fresh v2 document when the column is empty/unreadable.
    pub fn parse_or_default(json: Option<&str>) -> TurnsDoc {
        json.and_then(TurnsDoc::parse).unwrap_or_default()
    }

    /// Serialise as version 2, whatever version was read.
    pub fn to_json(&self) -> String {
        let mut out = self.clone();
        out.version = WRITE_VERSION;
        serde_json::to_string(&out).unwrap_or_else(|_| "{\"version\":2,\"turns\":[]}".into())
    }

    pub fn state(&self, pass: Pass) -> Option<PassState> {
        self.passes.get(pass)
    }

    pub fn set_state(&mut self, pass: Pass, state: PassState) {
        *self.passes.slot(pass) = Some(state);
        if state != PassState::Failed {
            self.errors.remove(pass.key());
        }
    }

    pub fn fail(&mut self, pass: Pass, error: impl Into<String>) {
        *self.passes.slot(pass) = Some(PassState::Failed);
        self.errors.insert(pass.key().to_string(), error.into());
    }

    /// Apply a speakers-pass outcome. A failure keeps whatever turns an
    /// earlier successful run produced: a failed retry never discards work.
    pub fn apply_speakers(&mut self, outcome: Result<(Vec<SpeakerTurn>, EngineInfo), String>) {
        match outcome {
            Ok((turns, engine)) => {
                self.turns = turns;
                self.engine = Some(engine);
                self.set_state(Pass::Speakers, PassState::Done);
            }
            Err(e) => self.fail(Pass::Speakers, e),
        }
    }

    /// Apply an accuracy-pass outcome, with the same keep-on-failure rule.
    pub fn apply_accuracy(&mut self, outcome: Result<Accuracy, String>) {
        match outcome {
            Ok(acc) => {
                self.accuracy = Some(acc);
                self.set_state(Pass::Accuracy, PassState::Done);
            }
            Err(e) => self.fail(Pass::Accuracy, e),
        }
    }

    /// Notes text itself lives in `meeting_notes_text`; the doc only records
    /// whether the pass worked.
    pub fn apply_notes(&mut self, outcome: Result<(), String>) {
        match outcome {
            Ok(()) => self.set_state(Pass::Notes, PassState::Done),
            Err(e) => self.fail(Pass::Notes, e),
        }
    }

    /// True when at least one transcript exists, so the row is usable even if
    /// some passes failed.
    pub fn has_transcript(&self) -> bool {
        !self.turns.is_empty() || self.accuracy.as_ref().is_some_and(|a| !a.text.trim().is_empty())
    }

    pub fn speaker_text(&self) -> Option<String> {
        (!self.turns.is_empty()).then(|| crate::stt::render_turns(&self.turns))
    }

    pub fn accuracy_text(&self) -> Option<&str> {
        self.accuracy
            .as_ref()
            .map(|a| a.text.as_str())
            .filter(|t| !t.trim().is_empty())
    }
}

/// When the provider returned a transcript but no speaker split (one voice, or
/// voices it could not tell apart), keep the text as a single "Speaker 1"
/// turn so the conversation still reads as one.
pub fn turns_or_single(turns: Option<Vec<SpeakerTurn>>, text: &str, duration: Option<f64>) -> Vec<SpeakerTurn> {
    match turns {
        Some(t) if !t.is_empty() => t,
        _ if text.trim().is_empty() => Vec::new(),
        _ => vec![SpeakerTurn {
            speaker: "Speaker 1".into(),
            text: text.trim().to_string(),
            start: Some(0.0),
            end: duration,
        }],
    }
}

/// Appended to the meeting-notes system prompt when a Whisper transcript is
/// sent alongside the speaker transcript.
pub const NOTES_REFERENCE_RULE: &str = "\n\nThe message may contain two transcripts of the same recording. The first (\"Speaker transcript\") knows who spoke but can mishear words. The second (\"Reference transcript (Whisper)\") has more accurate wording but no speakers. Attribute from the speaker transcript; use the reference only to correct words, names and numbers.";

/// The user message for the notes pass. `None` when there is nothing to
/// summarise.
pub fn notes_input(speakers: Option<&str>, accuracy: Option<&str>) -> Option<String> {
    let speakers = speakers.map(str::trim).filter(|s| !s.is_empty());
    let accuracy = accuracy.map(str::trim).filter(|s| !s.is_empty());
    match (speakers, accuracy) {
        (Some(s), Some(a)) => Some(format!(
            "## Speaker transcript\n\n{s}\n\n## Reference transcript (Whisper)\n\n{a}"
        )),
        (Some(s), None) => Some(s.to_string()),
        (None, Some(a)) => Some(a.to_string()),
        (None, None) => None,
    }
}

// ─── Export ─────────────────────────────────────────────────────────────────

pub const NOT_GENERATED: &str = "(not generated)";

const EXPORT_NOTE: &str = "> Two transcripts of the same recording follow. The speaker transcript\n> knows who spoke but can mishear words. The Whisper transcript has more\n> accurate wording but no speakers. Use the speaker transcript for who said\n> what, and the Whisper transcript to correct the wording.";

pub fn pretty_provider(id: &str) -> &str {
    match id {
        "deepgram" => "Deepgram",
        "elevenlabs" => "ElevenLabs",
        "gemini" => "Gemini",
        "groq" => "Groq",
        "openai" => "OpenAI",
        other => other,
    }
}

/// `mm:ss` from seconds (minutes are not wrapped past 59, so a 75-minute
/// meeting reads `[75:12]`).
pub fn mm_ss(secs: f64) -> String {
    let total = secs.max(0.0).floor() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// `h:mm:ss` from milliseconds.
pub fn h_mm_ss(ms: i64) -> String {
    let total = (ms.max(0) / 1000) as u64;
    format!("{}:{:02}:{:02}", total / 3600, (total / 60) % 60, total % 60)
}

/// The display name for a speaker label.
pub fn named<'a>(label: &'a str, names: &'a BTreeMap<String, String>) -> &'a str {
    names
        .get(label)
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
        .unwrap_or(label)
}

/// Replace whole-label occurrences of "Speaker N" with the user's names, so
/// "Speaker 1" never matches inside "Speaker 12".
pub fn substitute_names(text: &str, names: &BTreeMap<String, String>) -> String {
    let mut labels: Vec<(&String, &str)> = names
        .iter()
        .map(|(k, v)| (k, v.trim()))
        .filter(|(k, v)| !v.is_empty() && !k.is_empty())
        .collect();
    // Longest label first so "Speaker 12" is handled before "Speaker 1".
    labels.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let mut out = text.to_string();
    for (label, name) in labels {
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(pos) = rest.find(label.as_str()) {
            let before = rest[..pos].chars().next_back();
            let after = rest[pos + label.len()..].chars().next();
            let boundary_before = before.is_none_or(|c| !c.is_alphanumeric());
            let boundary_after = after.is_none_or(|c| !c.is_alphanumeric());
            result.push_str(&rest[..pos]);
            if boundary_before && boundary_after {
                result.push_str(name);
            } else {
                result.push_str(label);
            }
            rest = &rest[pos + label.len()..];
        }
        result.push_str(rest);
        out = result;
    }
    out
}

/// Distinct speaker labels in first-seen order.
pub fn speaker_labels(turns: &[SpeakerTurn]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for t in turns {
        if !seen.contains(&t.speaker) {
            seen.push(t.speaker.clone());
        }
    }
    seen
}

/// Everything the export needs, already resolved by the caller.
pub struct ExportInput<'a> {
    pub title: Option<&'a str>,
    /// Shown on the Date line, e.g. "2026-10-19 14:05".
    pub date_display: String,
    pub duration_ms: i64,
    pub doc: &'a TurnsDoc,
    pub names: &'a BTreeMap<String, String>,
    pub notes: Option<&'a str>,
    /// Used for the Speaker engine line when the doc has no `engine` (v1 rows).
    pub stt_provider_fallback: Option<&'a str>,
}

pub fn display_title(title: Option<&str>) -> String {
    title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("Conversation")
        .to_string()
}

/// The exact Markdown format from the contract.
pub fn export_markdown(input: &ExportInput) -> String {
    let doc = input.doc;
    let names = input.names;
    let title = display_title(input.title);

    let labels = speaker_labels(&doc.turns);
    let speakers_line = if labels.is_empty() {
        NOT_GENERATED.to_string()
    } else {
        labels.iter().map(|l| named(l, names)).collect::<Vec<_>>().join(", ")
    };
    let speaker_engine = match (&doc.engine, input.stt_provider_fallback) {
        (Some(e), _) => format!("{} {}", pretty_provider(&e.provider), e.model),
        (None, Some(p)) if !p.trim().is_empty() && !doc.turns.is_empty() => pretty_provider(p).to_string(),
        _ => NOT_GENERATED.to_string(),
    };
    let word_engine = match &doc.accuracy {
        Some(a) if !a.text.trim().is_empty() => format!("{} {}", pretty_provider(&a.provider), a.model),
        _ => NOT_GENERATED.to_string(),
    };

    let notes = input
        .notes
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| demote_headings(&substitute_names(n, names)))
        .unwrap_or_else(|| NOT_GENERATED.to_string());

    let transcript = if doc.turns.is_empty() {
        NOT_GENERATED.to_string()
    } else {
        doc.turns
            .iter()
            .map(|t| {
                let who = named(&t.speaker, names);
                match t.start {
                    Some(s) => format!("**[{}] {}:** {}", mm_ss(s), who, t.text.trim()),
                    None => format!("**{}:** {}", who, t.text.trim()),
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let whisper = doc
        .accuracy_text()
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| NOT_GENERATED.to_string());

    format!(
        "# {title}\n\n- Date: {date}\n- Length: {length}\n- Speakers: {speakers_line}\n- Speaker engine: {speaker_engine}\n- Word engine: {word_engine}\n\n{EXPORT_NOTE}\n\n## Meeting notes\n\n{notes}\n\n## Speaker transcript\n\n{transcript}\n\n## Whisper transcript\n\n{whisper}\n",
        date = input.date_display,
        length = h_mm_ss(input.duration_ms),
    )
}

/// Push Markdown headings in meeting notes one level down, so the notes'
/// own "## Summary" sits under the export's "## Meeting notes".
pub fn demote_headings(text: &str) -> String {
    text.lines()
        .map(|line| {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            if (1..=5).contains(&hashes) && line[hashes..].starts_with(' ') {
                format!("#{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `<YYYY-MM-DD> <title>.md`, with the title made safe for every file system.
pub fn export_filename(date_ymd: &str, title: Option<&str>) -> String {
    let raw = display_title(title);
    let mut cleaned: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cleaned = cleaned.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if cleaned.chars().count() > 80 {
        cleaned = cleaned.chars().take(80).collect::<String>().trim_end().to_string();
    }
    if cleaned.is_empty() {
        cleaned = "Conversation".into();
    }
    format!("{date_ymd} {cleaned}.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_headings_are_demoted_below_the_export_section() {
        assert_eq!(
            demote_headings("## Summary\n- a\n#hashtag\n###### deep"),
            "### Summary\n- a\n#hashtag\n###### deep"
        );
    }

    fn turn(speaker: &str, text: &str, start: f64) -> SpeakerTurn {
        SpeakerTurn { speaker: speaker.into(), text: text.into(), start: Some(start), end: None }
    }

    #[test]
    fn reads_a_v1_envelope() {
        let doc = TurnsDoc::parse(
            r#"{"version":1,"turns":[{"speaker":"Speaker 1","text":"hi","start":0.5}]}"#,
        )
        .expect("v1 must parse");
        assert_eq!(doc.version, 1);
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].start, Some(0.5));
        assert_eq!(doc.turns[0].end, None);
        assert!(doc.accuracy.is_none());
        assert!(doc.engine.is_none());
        assert_eq!(doc.passes, Passes::default());
    }

    #[test]
    fn v1_turn_without_start_reads_as_unknown() {
        let doc = TurnsDoc::parse(r#"{"version":1,"turns":[{"speaker":"Speaker 1","text":"hi"}]}"#)
            .unwrap();
        assert_eq!(doc.turns[0].start, None);
    }

    #[test]
    fn v1_round_trips_as_v2_with_turns_intact() {
        let v1 = r#"{"version":1,"turns":[{"speaker":"Speaker 2","text":"yo","start":3.0}]}"#;
        let doc = TurnsDoc::parse(v1).unwrap();
        let out = doc.to_json();
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["version"], 2);
        assert_eq!(value["turns"][0]["speaker"], "Speaker 2");
        assert_eq!(value["turns"][0]["start"], 3.0);
        assert!(value.get("errors").is_none(), "empty fields are omitted");
        let back = TurnsDoc::parse(&out).unwrap();
        assert_eq!(back.turns, doc.turns);
    }

    #[test]
    fn reads_and_round_trips_the_full_v2_contract_example() {
        let v2 = r#"{
          "version": 2,
          "turns": [{ "speaker": "Speaker 1", "text": "…", "start": 0.0, "end": 4.2 }],
          "engine": { "provider": "deepgram", "model": "nova-3" },
          "accuracy": { "text": "Whisper's full transcript …", "provider": "groq", "model": "whisper-large-v3" },
          "passes": { "speakers": "done", "accuracy": "done", "notes": "failed" },
          "errors": { "notes": "Groq cleanup 429: rate limit" },
          "future_key": { "kept": true }
        }"#;
        let doc = TurnsDoc::parse(v2).unwrap();
        assert_eq!(doc.turns[0].end, Some(4.2));
        assert_eq!(doc.engine.as_ref().unwrap().model, "nova-3");
        assert_eq!(doc.accuracy.as_ref().unwrap().provider, "groq");
        assert_eq!(doc.state(Pass::Notes), Some(PassState::Failed));
        assert_eq!(doc.errors["notes"], "Groq cleanup 429: rate limit");
        let back = TurnsDoc::parse(&doc.to_json()).unwrap();
        assert_eq!(back, doc);
        assert_eq!(back.extra["future_key"]["kept"], true, "unknown keys survive");
    }

    #[test]
    fn rejects_unknown_versions_and_garbage() {
        assert!(TurnsDoc::parse(r#"{"version":3,"turns":[]}"#).is_none());
        assert!(TurnsDoc::parse(r#"{"turns":[]}"#).is_none());
        assert!(TurnsDoc::parse("not json").is_none());
        let fresh = TurnsDoc::parse_or_default(Some("not json"));
        assert_eq!(fresh.version, 2);
        assert!(fresh.turns.is_empty());
    }

    #[test]
    fn a_failed_pass_keeps_the_passes_that_worked() {
        let mut doc = TurnsDoc::default();
        doc.apply_speakers(Err("Deepgram 500".into()));
        doc.apply_accuracy(Ok(Accuracy {
            text: "hello world".into(),
            provider: "groq".into(),
            model: "whisper-large-v3".into(),
        }));
        doc.apply_notes(Ok(()));
        assert_eq!(doc.state(Pass::Speakers), Some(PassState::Failed));
        assert_eq!(doc.state(Pass::Accuracy), Some(PassState::Done));
        assert_eq!(doc.state(Pass::Notes), Some(PassState::Done));
        assert_eq!(doc.errors.len(), 1);
        assert_eq!(doc.errors["speakers"], "Deepgram 500");
        assert!(doc.has_transcript(), "Whisper alone still makes the row usable");
    }

    #[test]
    fn a_failed_retry_never_discards_an_earlier_success() {
        let mut doc = TurnsDoc::default();
        let engine = EngineInfo { provider: "deepgram".into(), model: "nova-3".into() };
        doc.apply_speakers(Ok((vec![turn("Speaker 1", "hi", 0.0)], engine)));
        doc.apply_speakers(Err("timeout".into()));
        assert_eq!(doc.turns.len(), 1, "earlier turns survive a failed rerun");
        assert_eq!(doc.state(Pass::Speakers), Some(PassState::Failed));

        doc.apply_accuracy(Ok(Accuracy { text: "a".into(), provider: "groq".into(), model: "m".into() }));
        doc.apply_accuracy(Err("429".into()));
        assert_eq!(doc.accuracy_text(), Some("a"));
    }

    #[test]
    fn a_successful_retry_clears_that_pass_error_only() {
        let mut doc = TurnsDoc::default();
        doc.apply_notes(Err("rate limit".into()));
        doc.apply_accuracy(Err("too big".into()));
        doc.apply_notes(Ok(()));
        assert_eq!(doc.state(Pass::Notes), Some(PassState::Done));
        assert!(!doc.errors.contains_key("notes"));
        assert_eq!(doc.errors["accuracy"], "too big");
    }

    #[test]
    fn all_passes_failed_means_no_transcript() {
        let mut doc = TurnsDoc::default();
        doc.apply_speakers(Err("a".into()));
        doc.apply_accuracy(Err("b".into()));
        assert!(!doc.has_transcript());
    }

    #[test]
    fn running_and_skipped_states_serialise_lowercase() {
        let mut doc = TurnsDoc::default();
        doc.set_state(Pass::Speakers, PassState::Running);
        doc.set_state(Pass::Accuracy, PassState::Skipped);
        let v: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        assert_eq!(v["passes"]["speakers"], "running");
        assert_eq!(v["passes"]["accuracy"], "skipped");
        assert!(v["passes"].get("notes").is_none());
    }

    #[test]
    fn single_speaker_fallback() {
        let t = turns_or_single(None, " just me ", Some(9.0));
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].speaker, "Speaker 1");
        assert_eq!(t[0].text, "just me");
        assert!(turns_or_single(None, "  ", None).is_empty());
    }

    #[test]
    fn notes_input_includes_the_whisper_reference_when_present() {
        assert_eq!(notes_input(None, None), None);
        assert_eq!(notes_input(Some("S1: hi"), None).unwrap(), "S1: hi");
        assert_eq!(notes_input(None, Some("hi")).unwrap(), "hi");
        let both = notes_input(Some("Speaker 1: hi"), Some("hi there")).unwrap();
        assert!(both.starts_with("## Speaker transcript\n\nSpeaker 1: hi"));
        assert!(both.ends_with("## Reference transcript (Whisper)\n\nhi there"));
    }

    fn names(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn export_matches_the_contract_format_exactly() {
        let mut doc = TurnsDoc::default();
        doc.apply_speakers(Ok((
            vec![
                turn("Speaker 1", "Shall we start?", 0.0),
                turn("Speaker 2", "Yes.", 42.3),
                turn("Speaker 3", "Hi all.", 61.0),
            ],
            EngineInfo { provider: "deepgram".into(), model: "nova-3".into() },
        )));
        doc.apply_accuracy(Ok(Accuracy {
            text: "Shall we start? Yes. Hi all.".into(),
            provider: "groq".into(),
            model: "whisper-large-v3".into(),
        }));
        let n = names(&[("Speaker 1", "Asha"), ("Speaker 2", "Rohan"), ("Speaker 3", "  ")]);
        let md = export_markdown(&ExportInput {
            title: Some("Weekly sync"),
            date_display: "2026-10-19 14:05".into(),
            duration_ms: 3_725_000,
            doc: &doc,
            names: &n,
            notes: Some("## Summary\n- Speaker 1 opened. Speaker 12 is not Speaker 1."),
            stt_provider_fallback: None,
        });
        let expected = "# Weekly sync\n\n- Date: 2026-10-19 14:05\n- Length: 1:02:05\n- Speakers: Asha, Rohan, Speaker 3\n- Speaker engine: Deepgram nova-3\n- Word engine: Groq whisper-large-v3\n\n> Two transcripts of the same recording follow. The speaker transcript\n> knows who spoke but can mishear words. The Whisper transcript has more\n> accurate wording but no speakers. Use the speaker transcript for who said\n> what, and the Whisper transcript to correct the wording.\n\n## Meeting notes\n\n### Summary\n- Asha opened. Speaker 12 is not Asha.\n\n## Speaker transcript\n\n**[00:00] Asha:** Shall we start?\n\n**[00:42] Rohan:** Yes.\n\n**[01:01] Speaker 3:** Hi all.\n\n## Whisper transcript\n\nShall we start? Yes. Hi all.\n";
        assert_eq!(md, expected);
    }

    #[test]
    fn export_shows_not_generated_for_missing_sections() {
        let doc = TurnsDoc::default();
        let n = BTreeMap::new();
        let md = export_markdown(&ExportInput {
            title: None,
            date_display: "2026-10-19 09:00".into(),
            duration_ms: 0,
            doc: &doc,
            names: &n,
            notes: Some("   "),
            stt_provider_fallback: None,
        });
        assert!(md.starts_with("# Conversation\n"));
        assert!(md.contains("- Speakers: (not generated)\n"));
        assert!(md.contains("- Speaker engine: (not generated)\n"));
        assert!(md.contains("- Word engine: (not generated)\n"));
        assert!(md.contains("## Meeting notes\n\n(not generated)\n\n## Speaker transcript"));
        assert!(md.contains("## Speaker transcript\n\n(not generated)\n\n## Whisper transcript"));
        assert!(md.ends_with("## Whisper transcript\n\n(not generated)\n"));
    }

    #[test]
    fn export_of_a_v1_row_uses_the_row_provider_and_untimed_turns() {
        let doc = TurnsDoc::parse(r#"{"version":1,"turns":[{"speaker":"Speaker 1","text":"hi"}]}"#)
            .unwrap();
        let n = BTreeMap::new();
        let md = export_markdown(&ExportInput {
            title: Some("Old"),
            date_display: "d".into(),
            duration_ms: 5_000,
            doc: &doc,
            names: &n,
            notes: None,
            stt_provider_fallback: Some("elevenlabs"),
        });
        assert!(md.contains("- Speaker engine: ElevenLabs\n"));
        assert!(md.contains("**Speaker 1:** hi"));
        assert!(md.contains("- Length: 0:00:05\n"));
    }

    #[test]
    fn export_filename_is_sanitised() {
        assert_eq!(export_filename("2026-10-19", Some("Weekly sync")), "2026-10-19 Weekly sync.md");
        assert_eq!(export_filename("2026-10-19", Some("Q3: plan / risks?")), "2026-10-19 Q3 plan risks.md");
        assert_eq!(export_filename("2026-10-19", Some("...")), "2026-10-19 Conversation.md");
        assert_eq!(export_filename("2026-10-19", None), "2026-10-19 Conversation.md");
        let long = "x".repeat(200);
        assert_eq!(export_filename("d", Some(&long)).len(), "d ".len() + 80 + ".md".len());
    }

    #[test]
    fn time_formats() {
        assert_eq!(mm_ss(0.0), "00:00");
        assert_eq!(mm_ss(42.9), "00:42");
        assert_eq!(mm_ss(4512.0), "75:12");
        assert_eq!(h_mm_ss(59_999), "0:00:59");
        assert_eq!(h_mm_ss(3_600_000), "1:00:00");
    }
}
