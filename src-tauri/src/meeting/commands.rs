//! Tauri commands for the meeting transcription feature.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::meeting::export::render;
use crate::meeting::history::History;
use crate::meeting::ingest::ingest_to_pcm_file;
use crate::meeting::jobs::{JobQueue, QueueSnapshot};
use crate::meeting::live::{LiveRecorder, LiveSource};
use crate::meeting::pipeline::{run, DiariseOpts, Engine, ProgressSink};
use crate::meeting::types::{
    ExportFormat, JobMode, JobState, Progress, Segment, SpeakerLabel, Transcript, TranscriptSource,
};

/// Held in Tauri-managed state for the whole module.
pub struct MeetingState {
    pub queue: Arc<JobQueue>,
    pub last_results: Arc<std::sync::Mutex<Vec<Transcript>>>,
    /// Persistent history. Lazily opened on first use against the app data dir.
    /// On open failure, this remains None and Phase 3.5 features no-op gracefully.
    history: std::sync::Mutex<Option<Arc<History>>>,
    /// Phase 3.2: in-flight live recordings keyed by their job_id. Each entry
    /// holds open capture threads + writers; removed on `meeting_stop_live`.
    live_recorders: std::sync::Mutex<std::collections::HashMap<Uuid, LiveRecorder>>,
}

impl MeetingState {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(JobQueue::new()),
            last_results: Arc::new(std::sync::Mutex::new(Vec::new())),
            history: std::sync::Mutex::new(None),
            live_recorders: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Lazily open the history DB under the app data dir. Returns `None` if the
    /// DB couldn't be opened (logged once); subsequent calls will retry.
    fn history_or_init<R: Runtime>(&self, app: &AppHandle<R>) -> Option<Arc<History>> {
        let mut slot = self.history.lock().unwrap();
        if let Some(h) = slot.as_ref() {
            return Some(h.clone());
        }
        let dir = settings_app_data_dir(app);
        match History::open(&dir) {
            Ok(h) => {
                let arc = Arc::new(h);
                *slot = Some(arc.clone());
                Some(arc)
            }
            Err(e) => {
                eprintln!("[iSpeak] meeting history unavailable: {e}");
                None
            }
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
    let history = state.history_or_init(&app);
    let app_clone = app.clone();
    tokio::spawn(async move {
        let _ = drive_worker(app_clone, queue, results, history).await;
    });
    Ok(id)
}

async fn drive_worker<R: Runtime>(
    app: AppHandle<R>,
    queue: Arc<JobQueue>,
    results: Arc<std::sync::Mutex<Vec<Transcript>>>,
    history: Option<Arc<History>>,
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
        let diarise = diarise_opts_from(&settings);
        let transcript_result = run(&pcm_path, source, engine, cancel.clone(), sink, diarise).await;

        let _ = std::fs::remove_dir_all(&cache_dir);

        match transcript_result {
            Ok(mut transcript) => {
                // Phase 3.4: summarise + extract action items. Non-fatal; we never block
                // delivery of the transcript on AI failure.
                queue.update_state(job_id, JobState::Summarizing);
                let _ = app.emit(
                    "meeting://progress",
                    ProgressEvent {
                        job_id,
                        state: "summarising".into(),
                        chunks_done: transcript.segments.len() as u32,
                        chunks_total: transcript.segments.len() as u32,
                    },
                );
                summarise_into(&mut transcript, &settings).await;

                if let Some(h) = &history {
                    if let Err(e) = h.persist(&transcript) {
                        eprintln!("[iSpeak] persist meeting history failed: {e}");
                    }
                }

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

/// Run AI summarisation on the transcript and populate `summary` + `action_items` in
/// place. Any failure (AI off, network down, parse error) is logged and ignored —
/// the transcript ships either way.
async fn summarise_into(transcript: &mut Transcript, settings: &crate::settings::AppSettings) {
    let plain = render_segments_plain(&transcript.segments);
    if plain.trim().is_empty() {
        return;
    }
    match crate::ai::summarize_meeting(&plain, &settings.ai_mode, settings).await {
        Ok(Some(summary)) => {
            transcript.summary = Some(summary.summary);
            transcript.action_items = summary.action_items;
        }
        Ok(None) => {
            // AI mode is Off — leave fields unset.
        }
        Err(e) => {
            eprintln!("[iSpeak] meeting summarisation failed: {e}");
        }
    }
}

fn render_segments_plain(segments: &[Segment]) -> String {
    let mut out = String::new();
    for seg in segments {
        let speaker = match &seg.speaker {
            SpeakerLabel::You => "You",
            SpeakerLabel::Other => "Speaker",
            SpeakerLabel::Indexed(n) => {
                out.push_str(&format!("Speaker {}: {}\n", (b'A' + *n) as char, seg.text));
                continue;
            }
        };
        out.push_str(&format!("{}: {}\n", speaker, seg.text));
    }
    out
}

fn diarise_opts_from(settings: &crate::settings::AppSettings) -> Option<DiariseOpts> {
    if !settings.auto_diarise {
        return None;
    }
    Some(DiariseOpts {
        k: settings.diarise_expected_speakers.max(1),
    })
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
pub fn meeting_export<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    transcript_id: Uuid,
    format: ExportFormat,
) -> Result<String> {
    // Try the in-memory cache first (cheaper, hot path).
    {
        let results = state.last_results.lock().unwrap();
        if let Some(t) = results.iter().find(|t| t.id == transcript_id) {
            return Ok(render(t, format));
        }
    }
    // Fall through to persistent history (Phase 3.5) — lets users export old meetings.
    if let Some(h) = state.history_or_init(&app) {
        if let Some(t) = h.get(transcript_id)? {
            return Ok(render(&t, format));
        }
    }
    Err(AppError::Meeting(format!(
        "transcript {transcript_id} not found"
    )))
}

// ─── Phase 3.5: persistent history ─────────────────────────────────────────

#[tauri::command]
pub fn meeting_list_history<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Transcript>> {
    let Some(h) = state.history_or_init(&app) else {
        return Ok(Vec::new());
    };
    h.list(query.as_deref(), limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn meeting_get_history<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    id: Uuid,
) -> Result<Option<Transcript>> {
    let Some(h) = state.history_or_init(&app) else {
        return Ok(None);
    };
    h.get(id)
}

#[tauri::command]
pub fn meeting_delete_history<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    id: Uuid,
) -> Result<bool> {
    let Some(h) = state.history_or_init(&app) else {
        return Ok(false);
    };
    h.delete(id)
}

// ─── Phase 3.2: live capture ───────────────────────────────────────────────

#[tauri::command]
pub fn meeting_start_live<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    source: LiveSource,
) -> Result<Uuid> {
    let settings = crate::commands::load_settings(&app);
    let mic_device_id = settings.microphone_id.clone();
    let job_id = Uuid::new_v4();
    let work_dir = std::env::temp_dir().join(format!("iSpeak-live-{job_id}"));

    let recorder = LiveRecorder::start(job_id, source, mic_device_id, work_dir.clone())?;

    state
        .live_recorders
        .lock()
        .unwrap()
        .insert(job_id, recorder);

    let _ = app.emit(
        "meeting://progress",
        ProgressEvent {
            job_id,
            state: "recording".into(),
            chunks_done: 0,
            chunks_total: 0,
        },
    );

    Ok(job_id)
}

#[tauri::command]
pub async fn meeting_stop_live<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    job_id: Uuid,
) -> Result<()> {
    let recorder = state
        .live_recorders
        .lock()
        .unwrap()
        .remove(&job_id)
        .ok_or_else(|| AppError::Meeting(format!("no live recording with id {job_id}")))?;

    let pcm_path = match tokio::task::spawn_blocking(move || recorder.stop_and_finalise()).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            let _ = app.emit(
                "meeting://error",
                ErrorEvent {
                    job_id,
                    reason: e.to_string(),
                },
            );
            return Err(e);
        }
        Err(e) => {
            let msg = format!("live finalise join error: {e}");
            let _ = app.emit(
                "meeting://error",
                ErrorEvent {
                    job_id,
                    reason: msg.clone(),
                },
            );
            return Err(AppError::Meeting(msg));
        }
    };

    let results = state.last_results.clone();
    let history = state.history_or_init(&app);
    let app_clone = app.clone();
    tokio::spawn(async move {
        process_live_pcm(app_clone, job_id, pcm_path, results, history).await;
    });
    Ok(())
}

/// Drive the transcription pipeline for a finalised live-capture PCM file.
/// Mirrors the post-ingest body of `drive_worker` but stays out of the JobQueue
/// (live captures are tracked separately).
async fn process_live_pcm<R: Runtime>(
    app: AppHandle<R>,
    job_id: Uuid,
    pcm_path: std::path::PathBuf,
    results: Arc<std::sync::Mutex<Vec<Transcript>>>,
    history: Option<Arc<History>>,
) {
    let settings = crate::commands::load_settings(&app);
    let engine = select_engine(&settings, &app);

    struct LiveProgress<R: Runtime> {
        app: AppHandle<R>,
        job_id: Uuid,
    }
    impl<R: Runtime> ProgressSink for LiveProgress<R> {
        fn on_chunk_done(&self, chunks_done: u32, chunks_total: u32) {
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
    let sink: Arc<dyn ProgressSink> = Arc::new(LiveProgress {
        app: app.clone(),
        job_id,
    });
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let source = TranscriptSource::LiveCapture;
    let diarise = diarise_opts_from(&settings);
    let transcript_result = run(&pcm_path, source, engine, cancel, sink, diarise).await;

    // Clean up the live work dir (parent of pcm_path).
    if let Some(parent) = pcm_path.parent() {
        LiveRecorder::cleanup(parent);
    }

    match transcript_result {
        Ok(mut transcript) => {
            let _ = app.emit(
                "meeting://progress",
                ProgressEvent {
                    job_id,
                    state: "summarising".into(),
                    chunks_done: transcript.segments.len() as u32,
                    chunks_total: transcript.segments.len() as u32,
                },
            );
            summarise_into(&mut transcript, &settings).await;

            if let Some(h) = &history {
                if let Err(e) = h.persist(&transcript) {
                    eprintln!("[iSpeak] persist live meeting failed: {e}");
                }
            }
            results.lock().unwrap().push(transcript.clone());
            let _ = app.emit("meeting://done", DoneEvent { job_id, transcript });
        }
        Err(e) => {
            let _ = app.emit(
                "meeting://error",
                ErrorEvent {
                    job_id,
                    reason: e.to_string(),
                },
            );
        }
    }
}

// ─── Phase 3.3: manual speaker relabel ─────────────────────────────────────

/// Update the speaker on a single segment. Mutates both the in-memory result cache
/// (so the live UI reflects the change immediately) and the persistent history row
/// (so refreshes and searches stay consistent). Returns true if anything was changed.
#[tauri::command]
pub fn meeting_set_segment_speaker<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, MeetingState>,
    transcript_id: Uuid,
    segment_index: usize,
    speaker: SpeakerLabel,
) -> Result<bool> {
    let mut changed_in_memory = false;
    // Live cache: mutate in place.
    {
        let mut results = state.last_results.lock().unwrap();
        if let Some(t) = results.iter_mut().find(|t| t.id == transcript_id) {
            let len = t.segments.len();
            let seg = t.segments.get_mut(segment_index).ok_or_else(|| {
                AppError::Meeting(format!(
                    "segment {segment_index} out of range (len {len})"
                ))
            })?;
            seg.speaker = speaker.clone();
            changed_in_memory = true;
        }
    }
    // Persistent history: load, mutate, re-persist. Whichever store the transcript
    // lives in, the update propagates.
    if let Some(h) = state.history_or_init(&app) {
        if let Some(mut t) = h.get(transcript_id)? {
            let len = t.segments.len();
            let seg = t.segments.get_mut(segment_index).ok_or_else(|| {
                AppError::Meeting(format!(
                    "segment {segment_index} out of range (history len {len})"
                ))
            })?;
            seg.speaker = speaker;
            h.persist(&t)?;
            return Ok(true);
        }
    }
    if !changed_in_memory {
        return Err(AppError::Meeting(format!(
            "transcript {transcript_id} not found"
        )));
    }
    Ok(true)
}
