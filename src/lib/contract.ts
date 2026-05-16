/**
 * iSpeak — Interface Contract
 *
 * READ-ONLY. Do not modify without an explicit architectural decision.
 *
 * This file is the single source of truth for the boundary between the
 * Rust backend (src-tauri/) and the React frontend (src/).
 *
 * - The Rust backend (commands.rs) implements every command listed here.
 * - The React frontend calls only functions exported from this file.
 * - Neither side may add, remove, or rename commands without updating this
 *   file first and having that change reviewed.
 *
 * AI coding agents: treat this file as read-only. If you believe a command
 * needs to change, propose the change in a comment and wait for human review.
 */

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

// ─────────────────────────────────────────────────────────────────────────────
// Shared Types
// ─────────────────────────────────────────────────────────────────────────────

/** Whether the user holds the hotkey (push-to-talk) or presses to toggle. */
export type RecordingMode = 'push_to_talk' | 'toggle'

/** Authoritative recording state. Lives in Rust; mirrored to frontend via events. */
export type RecordingState = 'idle' | 'recording' | 'processing'

/** Which engine performs speech-to-text. */
export type TranscriptionEngine = 'local' | 'groq'

/** Whether and how AI post-processing is applied after transcription. */
export type AIMode = 'off' | 'local' | 'cloud_fast' | 'cloud_quality'


/** Local Whisper model size. Larger = slower + more accurate. */
export type WhisperModel = 'tiny' | 'base' | 'small' | 'medium' | 'large'

/** Result returned after a dictation recording is transcribed and processed. */
export interface TranscriptResult {
  /** Final text after AI post-processing (same as raw_text when AI mode is off). */
  text: string
  /** Raw Whisper output before any AI post-processing. */
  raw_text: string
  /** How long the audio recording was, in milliseconds. */
  duration_ms: number
  /** Which engine produced the transcript. */
  engine: TranscriptionEngine
  /** Which AI mode was active. */
  ai_mode: AIMode
  /** ISO 8601 timestamp of when the transcript was produced. */
  timestamp: string
}

/** Full application settings. Persisted via tauri-plugin-store. */
export interface AppSettings {
  /** Keyboard shortcut string, e.g. "CommandOrControl+Shift+Space". */
  hotkey: string
  recording_mode: RecordingMode
  transcription_engine: TranscriptionEngine
  whisper_model: WhisperModel
  ai_mode: AIMode
  /** Ollama model name, e.g. "phi3.5" or "llama3.2:3b". */
  ollama_model: string
  /** Ollama server base URL. Default: "http://localhost:11434". */
  ollama_base_url: string
  /** API keys — returned masked (e.g. "sk-...xxxx") on read. Write full key to update. */
  groq_api_key: string
  /** Device ID from listMicrophones(), or null to use system default. */
  microphone_id: string | null
  /** Screen position of the floating recording indicator. */
  indicator_position: { x: number; y: number }
  /** Maximum single recording duration in seconds. Min: 5, Max: 300. Default: 60. */
  max_recording_duration_s: number
  /** Whether dark mode is forced (true) or follows system preference (false). */
  dark_mode: boolean
}

/** A microphone device available on the system. */
export interface MicrophoneDevice {
  id: string
  name: string
  is_default: boolean
}

/** Progress event emitted during Whisper model download. */
export interface ModelDownloadProgress {
  model: WhisperModel
  bytes_downloaded: number
  bytes_total: number
  /** 0–100 */
  percent: number
  complete: boolean
  error: string | null
}

/** A stored meeting transcript with structured output. */
export interface MeetingTranscript {
  id: string
  /** Original filename of the imported audio/video file. */
  file_name: string
  duration_ms: number
  segments: TranscriptSegment[]
  /** ~200-word LLM-generated summary. Empty string if AI mode was off. */
  summary: string
  /** Bullet-point action items extracted by LLM. Empty array if AI mode was off. */
  action_items: string[]
  created_at: string
}

/** A single timestamped segment within a meeting transcript. */
export interface TranscriptSegment {
  /** Speaker label from diarization, e.g. "Speaker 1". Null if diarization unavailable. */
  speaker: string | null
  start_ms: number
  end_ms: number
  text: string
}

/** Progress event emitted during file transcription. */
export interface TranscribeFileProgress {
  /** 0–100 */
  percent: number
  stage: 'chunking' | 'transcribing' | 'diarizing' | 'summarising'
}

/** Generic application error emitted via Tauri event. */
export interface AppError {
  code: string
  message: string
}

// ─────────────────────────────────────────────────────────────────────────────
// Dictation Commands
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Begin a recording session.
 * In push-to-talk mode, call this on hotkey down.
 * In toggle mode, call this on the first hotkey press.
 * Emits: recording_state_changed('recording')
 */
export const startRecording = (): Promise<void> =>
  invoke('start_recording')

/**
 * End the current recording session and begin transcription.
 * In push-to-talk mode, call this on hotkey up.
 * In toggle mode, call this on the second hotkey press.
 * Emits: recording_state_changed('processing'), then transcript_ready or app_error.
 */
export const stopRecording = (): Promise<TranscriptResult> =>
  invoke('stop_recording')

/**
 * Abort the current recording without transcribing.
 * Used when recording duration is below the minimum threshold (< 0.5s).
 * Emits: recording_state_changed('idle')
 */
export const cancelRecording = (): Promise<void> =>
  invoke('cancel_recording')

// ─────────────────────────────────────────────────────────────────────────────
// Settings Commands
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Retrieve the current application settings.
 * API key fields are returned masked.
 */
export const getSettings = (): Promise<AppSettings> =>
  invoke('get_settings')

/**
 * Update one or more settings fields.
 * Partial update — only provided fields are changed.
 * Hotkey changes take effect immediately (re-registers global shortcut).
 */
export const updateSettings = (settings: Partial<AppSettings>): Promise<void> =>
  invoke('update_settings', { settings })

// ─────────────────────────────────────────────────────────────────────────────
// Audio Commands
// ─────────────────────────────────────────────────────────────────────────────

/**
 * List all available microphone input devices on the system.
 */
export const listMicrophones = (): Promise<MicrophoneDevice[]> =>
  invoke('list_microphones')

// ─────────────────────────────────────────────────────────────────────────────
// Whisper Model Commands
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Returns the list of Whisper models already downloaded and available locally.
 */
export const getInstalledModels = (): Promise<WhisperModel[]> =>
  invoke('get_installed_models')

/**
 * Begin downloading a Whisper model to $APP_DATA/models/.
 * Progress reported via model_download_progress events.
 */
export const downloadModel = (model: WhisperModel): Promise<void> =>
  invoke('download_model', { model })

/**
 * Delete a locally installed Whisper model to free disk space.
 * Will error if the specified model is currently selected in settings.
 */
export const deleteModel = (model: WhisperModel): Promise<void> =>
  invoke('delete_model', { model })

// ─────────────────────────────────────────────────────────────────────────────
// Meeting / Transcribe Commands (Phase 3)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Transcribe an audio or video file.
 * Accepted: .mp4, .m4a, .mp3, .wav, .ogg
 * Progress reported via transcribe_file_progress events.
 * Result stored in SQLite history and returned.
 */
export const transcribeFile = (filePath: string): Promise<MeetingTranscript> =>
  invoke('transcribe_file', { filePath })

/**
 * Retrieve stored meeting transcripts, newest first.
 */
export const getMeetingHistory = (
  limit: number = 50,
  offset: number = 0
): Promise<MeetingTranscript[]> =>
  invoke('get_meeting_history', { limit, offset })

/**
 * Delete a stored meeting transcript from history.
 */
export const deleteMeetingTranscript = (id: string): Promise<void> =>
  invoke('delete_meeting_transcript', { id })

/**
 * Export a stored transcript to a formatted string.
 * 'markdown' returns a Markdown document string ready to write to file.
 */
export const exportTranscript = (
  id: string,
  format: 'markdown'
): Promise<string> =>
  invoke('export_transcript', { id, format })

// ─────────────────────────────────────────────────────────────────────────────
// Events — Rust → Frontend
// All event listeners return an unlisten function. Call it on component unmount.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Emitted whenever the recording state machine transitions.
 * Frontend should update UI to reflect the new state.
 */
export const onRecordingStateChanged = (
  handler: (state: RecordingState) => void
): Promise<UnlistenFn> =>
  listen<RecordingState>('recording_state_changed', e => handler(e.payload))

/**
 * Emitted after transcription and AI post-processing complete.
 * The text has already been pasted to the cursor by the time this fires.
 * Frontend can use this to show a confirmation or log to history.
 */
export const onTranscriptReady = (
  handler: (result: TranscriptResult) => void
): Promise<UnlistenFn> =>
  listen<TranscriptResult>('transcript_ready', e => handler(e.payload))

/**
 * Emitted repeatedly during a Whisper model download.
 * Use to drive a progress bar in the settings panel.
 */
export const onModelDownloadProgress = (
  handler: (progress: ModelDownloadProgress) => void
): Promise<UnlistenFn> =>
  listen<ModelDownloadProgress>('model_download_progress', e => handler(e.payload))

/**
 * Emitted repeatedly during audio file transcription (Phase 3).
 * Use to drive a progress indicator in the meeting view.
 */
export const onTranscribeFileProgress = (
  handler: (progress: TranscribeFileProgress) => void
): Promise<UnlistenFn> =>
  listen<TranscribeFileProgress>('transcribe_file_progress', e => handler(e.payload))

/**
 * Emitted when a recoverable error occurs (e.g. transcription failed, model not found).
 * Display a toast or inline error message. App returns to IDLE after emitting this.
 */
export const onError = (
  handler: (error: AppError) => void
): Promise<UnlistenFn> =>
  listen<AppError>('app_error', e => handler(e.payload))

// ─── Meeting transcription (Phase 3) ────────────────────────────────────────

export interface MeetingSegment {
  start: number
  end: number
  speaker: { kind: 'you' } | { kind: 'other' } | { kind: 'indexed'; value: number }
  text: string
}

export interface MeetingProgress {
  job_id: string
  state: string
  chunks_done: number
  chunks_total: number
}

export interface MeetingDoneEvent {
  job_id: string
  transcript: MeetingTranscript
}

export interface MeetingErrorEvent {
  job_id: string
  reason: string
}

export type ExportFormat = 'markdown' | 'plain_text' | 'json' | 'srt' | 'vtt'

export const meetingEnqueueFile = (path: string): Promise<string> =>
  invoke('meeting_enqueue_file', { path })

export const meetingCancel = (jobId: string): Promise<boolean> =>
  invoke('meeting_cancel', { jobId })

export const meetingExport = (transcriptId: string, format: ExportFormat): Promise<string> =>
  invoke('meeting_export', { transcriptId, format })

export const onMeetingProgress = (cb: (p: MeetingProgress) => void): Promise<UnlistenFn> =>
  listen<MeetingProgress>('meeting://progress', (e) => cb(e.payload))

export const onMeetingDone = (cb: (e: MeetingDoneEvent) => void): Promise<UnlistenFn> =>
  listen<MeetingDoneEvent>('meeting://done', (e) => cb(e.payload))

export const onMeetingError = (cb: (e: MeetingErrorEvent) => void): Promise<UnlistenFn> =>
  listen<MeetingErrorEvent>('meeting://error', (e) => cb(e.payload))
