//! Phase 3 — meeting transcription module.
//! See docs/superpowers/specs/2026-05-16-phase-3-meeting-transcription-design.md

pub mod chunker;
pub mod commands;
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
