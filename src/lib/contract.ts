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
  /** Phase 3.3b: run heuristic speaker diarisation on meeting transcripts. */
  auto_diarise: boolean
  /** Expected number of distinct speakers (k for k-means). Default 2. */
  diarise_expected_speakers: number
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
 * Emitted when a recoverable error occurs (e.g. transcription failed, model not found).
 * Display a toast or inline error message. App returns to IDLE after emitting this.
 */
export const onError = (
  handler: (error: AppError) => void
): Promise<UnlistenFn> =>
  listen<AppError>('app_error', e => handler(e.payload))

// ─── Meeting transcription (Phase 3) ────────────────────────────────────────

export interface MeetingSegment {
  start: number    // seconds
  end: number      // seconds
  speaker: { kind: 'you' } | { kind: 'other' } | { kind: 'indexed'; value: number }
  text: string
}

/** A stored meeting transcript — matches the Rust `Transcript` struct exactly. */
export interface MeetingTranscript {
  id: string
  created_at: number    // unix millis
  duration_secs: number
  source:
    | { kind: 'file_import'; value: string }
    | { kind: 'live_capture' }
  segments: MeetingSegment[]
  summary: string | null
  action_items: string[]
  partial: boolean
  /** User-facing label. Auto-generated from summary on first save; renamable. */
  title: string | null
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

// ─── Phase 3.5: persistent history ─────────────────────────────────────────

export interface MeetingHistoryListOpts {
  query?: string | null
  limit?: number
  offset?: number
}

export const meetingListHistory = (opts: MeetingHistoryListOpts = {}): Promise<MeetingTranscript[]> =>
  invoke('meeting_list_history', {
    query: opts.query ?? null,
    limit: opts.limit ?? null,
    offset: opts.offset ?? null,
  })

export const meetingGetHistory = (id: string): Promise<MeetingTranscript | null> =>
  invoke('meeting_get_history', { id })

export const meetingDeleteHistory = (id: string): Promise<boolean> =>
  invoke('meeting_delete_history', { id })

// ─── Phase 3.2: live capture ───────────────────────────────────────────────

export type LiveSource = 'mic_only' | 'system_only' | 'mic_and_system'

export const meetingStartLive = (source: LiveSource): Promise<string> =>
  invoke('meeting_start_live', { source })

export const meetingStopLive = (jobId: string): Promise<void> =>
  invoke('meeting_stop_live', { jobId })

// ─── Polish #1: rename a meeting ───────────────────────────────────────────

export const meetingSetTitle = (id: string, title: string | null): Promise<boolean> =>
  invoke('meeting_set_title', { id, title })

// ─── Polish #2: re-summarise an existing transcript ────────────────────────

export const meetingResummarise = (id: string): Promise<MeetingTranscript> =>
  invoke('meeting_resummarise', { id })

// ─── Phase 3.3: manual speaker relabel ─────────────────────────────────────

export const meetingSetSegmentSpeaker = (
  transcriptId: string,
  segmentIndex: number,
  speaker: MeetingSegment['speaker'],
): Promise<boolean> =>
  invoke('meeting_set_segment_speaker', {
    transcriptId,
    segmentIndex,
    speaker,
  })

/// Cycle a speaker label: You → Speaker → A → B → … → Z → You.
export function nextSpeakerLabel(
  current: MeetingSegment['speaker'],
): MeetingSegment['speaker'] {
  switch (current.kind) {
    case 'you':
      return { kind: 'other' }
    case 'other':
      return { kind: 'indexed', value: 0 }
    case 'indexed':
      if (current.value >= 25) return { kind: 'you' }
      return { kind: 'indexed', value: current.value + 1 }
  }
}
