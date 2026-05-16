//! Tauri commands for the meeting transcription feature.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::meeting::export::render;
use crate::meeting::ingest::ingest_to_pcm_file;
use crate::meeting::jobs::{JobQueue, QueueSnapshot};
use crate::meeting::pipeline::{run, Engine, ProgressSink};
use crate::meeting::types::{ExportFormat, JobMode, JobState, Progress, Transcript, TranscriptSource};

/// Held in Tauri-managed state for the whole module.
pub struct MeetingState {
    pub queue: Arc<JobQueue>,
    pub last_results: Arc<std::sync::Mutex<Vec<Transcript>>>,
}

impl MeetingState {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(JobQueue::new()),
            last_results: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

#[derive(Clone, Serialize)]
struct ProgressEvent {
    job_id: Uuid,
    state: String,
    chunks_done: u32,
    chunks_total: u32,
}

#[derive(Clone, Serialize)]
struct DoneEvent {
    job_id: Uuid,
    transcript: Transcript,
}

#[derive(Clone, Serialize)]
struct ErrorEvent {
    job_id: Uuid,
    reason: String,
}

struct EmitProgress<R: Runtime> {
    app: AppHandle<R>,
    job_id: Uuid,
    queue: Arc<JobQueue>,
}

impl<R: Runtime> ProgressSink for EmitProgress<R> {
    fn on_chunk_done(&self, chunks_done: u32, chunks_total: u32) {
        self.queue.update_progress(self.job_id, Progress { chunks_done, chunks_total });
        let _ = self.app.emit(
            "meeting://progress",
            ProgressEvent {
                job_id: self.job_id,
                state: "transcribing".into(),
                chunks_done,
                chunks_total,
            },
        );
    }
}

#[tauri::command]
pub async fn meeting_enqueue_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    path: PathBuf,
) -> Result<Uuid> {
    if !path.exists() {
        return Err(AppError::Meeting(format!("file not found: {}", path.display())));
    }
    let id = state.queue.enqueue(JobMode::FileImport { path: path.clone() });
    let queue = state.queue.clone();
    let results = state.last_results.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        let _ = drive_worker(app_clone, queue, results).await;
    });
    Ok(id)
}

async fn drive_worker<R: Runtime>(
    app: AppHandle<R>,
    queue: Arc<JobQueue>,
    results: Arc<std::sync::Mutex<Vec<Transcript>>>,
) -> Result<()> {
    loop {
        let Some((job, cancel)) = queue.start_next() else { return Ok(()); };
        let job_id = job.id;
        let JobMode::FileImport { path } = job.mode.clone() else {
            queue.finish_running(JobState::Error("live capture not implemented in 3.1".into()));
            continue;
        };

        // Ingest
        let cache_dir = std::env::temp_dir().join(format!("iSpeak-jobs-{job_id}"));
        let pcm_path = match ingest_to_pcm_file(&path, &cache_dir) {
            Ok(p) => p,
            Err(e) => {
                let _ = app.emit("meeting://error", ErrorEvent { job_id, reason: e.to_string() });
                queue.finish_running(JobState::Error(e.to_string()));
                continue;
            }
        };

        // Engine selection from settings
        // Fields: settings.transcription_engine (TranscriptionEngine::Local / ::Groq)
        //         settings.whisper_model (WhisperModel)
        //         settings.groq_api_key (String, not Option)
        let settings = crate::commands::load_settings(&app);
        let engine = select_engine(&settings, &app);

        // Run pipeline
        let sink: Arc<dyn ProgressSink> = Arc::new(EmitProgress {
            app: app.clone(),
            job_id,
            queue: queue.clone(),
        });
        let source = TranscriptSource::FileImport(path.clone());
        let transcript_result = run(&pcm_path, source, engine, cancel.clone(), sink).await;

        let _ = std::fs::remove_dir_all(&cache_dir);

        match transcript_result {
            Ok(transcript) => {
                results.lock().unwrap().push(transcript.clone());
                let _ = app.emit("meeting://done", DoneEvent { job_id, transcript });
                queue.finish_running(JobState::AwaitingUserSave);
            }
            Err(e) => {
                let _ = app.emit("meeting://error", ErrorEvent { job_id, reason: e.to_string() });
                queue.finish_running(JobState::Error(e.to_string()));
            }
        }
    }
}

fn select_engine<R: Runtime>(settings: &crate::settings::AppSettings, app: &AppHandle<R>) -> Engine {
    match settings.transcription_engine {
        crate::settings::TranscriptionEngine::Local => Engine::Local {
            app_data_dir: settings_app_data_dir(app),
            model: settings.whisper_model.clone(),
        },
        crate::settings::TranscriptionEngine::Groq => Engine::GroqCloud {
            api_key: settings.groq_api_key.clone(),
        },
    }
}

fn settings_app_data_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
}

#[tauri::command]
pub fn meeting_cancel(state: State<'_, MeetingState>, job_id: Uuid) -> Result<bool> {
    Ok(state.queue.cancel(job_id))
}

#[tauri::command]
pub fn meeting_queue_snapshot(state: State<'_, MeetingState>) -> Result<QueueSnapshot> {
    Ok(state.queue.snapshot())
}

#[tauri::command]
pub fn meeting_export(
    state: State<'_, MeetingState>,
    transcript_id: Uuid,
    format: ExportFormat,
) -> Result<String> {
    let results = state.last_results.lock().unwrap();
    let t = results
        .iter()
        .find(|t| t.id == transcript_id)
        .ok_or_else(|| AppError::Meeting(format!("transcript {transcript_id} not found")))?;
    Ok(render(t, format))
}
