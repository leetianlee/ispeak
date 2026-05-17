//! Phase 3 — meeting transcription module.
//! See docs/superpowers/specs/2026-05-16-phase-3-meeting-transcription-design.md

pub mod chunker;
pub mod commands;
pub mod diarise;
pub mod export;
pub mod history;
pub mod ingest;
pub mod jobs;
pub mod live;
#[cfg(target_os = "macos")]
pub mod live_macos;
pub mod pipeline;
pub mod stitch;
pub mod types;

pub use types::{
    ExportFormat, Job, JobMode, JobState, Progress, Segment, SpeakerLabel,
    Transcript, TranscriptSource,
};

pub use commands::MeetingState;

/// Length cap for auto-generated titles.
const AUTO_TITLE_MAX_LEN: usize = 60;

/// Populate `transcript.title` if it's currently empty. Prefers the first
/// sentence of the summary; falls back to the first segment's text. No-op
/// when a non-empty title is already set (user-edited or previously derived).
pub fn derive_title_if_empty(transcript: &mut Transcript) {
    if transcript
        .title
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return;
    }
    if let Some(derived) = derive_title(transcript) {
        transcript.title = Some(derived);
    }
}

fn derive_title(t: &Transcript) -> Option<String> {
    // Prefer the summary: meaningful, already a sentence.
    if let Some(summary) = &t.summary {
        if let Some(line) = first_meaningful_line(summary) {
            return Some(ellipsize(&line, AUTO_TITLE_MAX_LEN));
        }
    }
    // Fall back to the first non-empty segment text.
    for seg in &t.segments {
        let text = seg.text.trim();
        if !text.is_empty() {
            return Some(ellipsize(text, AUTO_TITLE_MAX_LEN));
        }
    }
    None
}

fn first_meaningful_line(s: &str) -> Option<String> {
    // First sentence terminator wins; otherwise first line.
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cut = trimmed
        .find(|c: char| c == '.' || c == '!' || c == '?' || c == '\n')
        .map(|i| i + 1)
        .unwrap_or(trimmed.len());
    let line = trimmed[..cut].trim_end_matches(|c: char| c == '.' || c == '!' || c == '?');
    let line = line.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max - 1).collect();
    let trimmed = truncated.trim_end();
    format!("{trimmed}…")
}

#[cfg(test)]
mod title_tests {
    use super::*;
    use crate::meeting::types::{Segment, SpeakerLabel, TranscriptSource};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture(summary: Option<&str>, first_seg: &str) -> Transcript {
        Transcript {
            id: Uuid::nil(),
            created_at: 0,
            duration_secs: 0.0,
            source: TranscriptSource::FileImport(PathBuf::new()),
            segments: vec![Segment {
                start: 0.0,
                end: 1.0,
                speaker: SpeakerLabel::Other,
                text: first_seg.into(),
            }],
            summary: summary.map(String::from),
            action_items: vec![],
            partial: false,
            title: None,
            speaker_names: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn derives_from_summary_first_sentence() {
        let mut t = fixture(
            Some("Standup about Q3 roadmap. Discussed two items."),
            "ignored",
        );
        derive_title_if_empty(&mut t);
        assert_eq!(t.title.as_deref(), Some("Standup about Q3 roadmap"));
    }

    #[test]
    fn falls_back_to_first_segment_when_no_summary() {
        let mut t = fixture(None, "Hey team, quick sync about the launch plan");
        derive_title_if_empty(&mut t);
        assert_eq!(
            t.title.as_deref(),
            Some("Hey team, quick sync about the launch plan")
        );
    }

    #[test]
    fn does_not_overwrite_existing_title() {
        let mut t = fixture(Some("New summary."), "ignored");
        t.title = Some("My meeting".into());
        derive_title_if_empty(&mut t);
        assert_eq!(t.title.as_deref(), Some("My meeting"));
    }

    #[test]
    fn treats_empty_title_as_missing() {
        let mut t = fixture(Some("Real title here."), "ignored");
        t.title = Some("   ".into());
        derive_title_if_empty(&mut t);
        assert_eq!(t.title.as_deref(), Some("Real title here"));
    }

    #[test]
    fn ellipsizes_long_titles() {
        let long = "a".repeat(200);
        let mut t = fixture(Some(&long), "ignored");
        derive_title_if_empty(&mut t);
        let title = t.title.unwrap();
        assert!(title.chars().count() <= AUTO_TITLE_MAX_LEN);
        assert!(title.ends_with('…'));
    }
}
