//! Shared types for the meeting transcription module.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum JobMode {
    FileImport { path: PathBuf },
    LiveCapture,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum JobState {
    Created,
    Queued,
    Recording,
    Transcribing,
    Diarizing,
    Summarizing,
    AwaitingUserSave,
    Saved,
    Discarded,
    Canceled,
    Error(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub chunks_done: u32,
    pub chunks_total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub mode: JobMode,
    pub state: JobState,
    pub created_at: u64,        // unix millis
    pub progress: Progress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum SpeakerLabel {
    You,
    Other,
    Indexed(u8),
}

impl SpeakerLabel {
    /// Canonical string key for use as a map key (e.g. in `speaker_names`).
    /// Matches the convention the frontend uses: "you", "other", "indexed:N".
    pub fn key(&self) -> String {
        match self {
            SpeakerLabel::You => "you".to_string(),
            SpeakerLabel::Other => "other".to_string(),
            SpeakerLabel::Indexed(n) => format!("indexed:{n}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start: f32,            // seconds
    pub end: f32,
    pub speaker: SpeakerLabel,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum TranscriptSource {
    FileImport(PathBuf),
    LiveCapture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: Uuid,
    pub created_at: u64,
    pub duration_secs: f32,
    pub source: TranscriptSource,
    pub segments: Vec<Segment>,
    pub summary: Option<String>,
    pub action_items: Vec<String>,
    pub partial: bool,
    /// User-facing label for the transcript. Auto-generated on first save from
    /// the summary's first line (or first ~50 chars of segment text if AI is off).
    /// User can rename via `meeting_set_title`.
    #[serde(default)]
    pub title: Option<String>,

    /// Polish #3: per-transcript custom names for speaker labels.
    /// Key is the canonical SpeakerLabel string ("you", "other", "indexed:0", …).
    /// When a key is present, the UI and Markdown export use the value as the
    /// display name instead of the default ("Speaker", "Speaker A", …).
    #[serde(default)]
    pub speaker_names: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Markdown,
    PlainText,
    Json,
    Srt,
    Vtt,
}
