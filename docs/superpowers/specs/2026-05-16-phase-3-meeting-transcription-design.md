# Phase 3 — Meeting Transcription Design

**Date:** 2026-05-16
**Status:** Approved, ready for implementation plan
**Covers:** SPEC.md Mode 2 (both sub-modes A and B)

## 1. Goal

Add meeting-grade transcription to iSpeak as a second mode alongside short-burst dictation. Two sub-modes:

- **A. File import** — drag/drop or pick an audio/video file, get transcript + speaker labels + summary + action items, export in multiple formats.
- **B. Live capture** — capture system audio + microphone during a meeting, show a rolling transcript, finalise with diarisation and summary on stop.

Both sub-modes share the same downstream pipeline once raw PCM exists. The mode lives in a new `Transcribe` tab in the existing app shell.

## 2. Decisions (locked)

| Area | Decision | Rationale |
|---|---|---|
| Scope | Both sub-modes A and B | Full picture before architecture commitment. |
| Transcription | whisper.cpp local (existing) + Groq Whisper cloud (existing) | No new transcription vendor. Reuse Phase 1 plumbing. |
| Diarisation | sherpa-onnx local (default) + Deepgram opt-in cloud | Local-by-default per PRODUCT.md. Deepgram is a quality upgrade, not a requirement. |
| Summary + action items | Cloud-only via Groq Llama 3.3 70B | Local 3B models too weak; honest about capability. Transcripts still work fully offline. |
| Live capture | ScreenCaptureKit (macOS 13+ native) | No driver install. Lower onboarding friction than BlackHole. |
| Live audio sources | Mic + system, captured to separate tracks | Reliable "me vs them" labelling without heuristic diarisation. |
| UI | New `Transcribe` tab with sub-sections: drop zone, in-progress, history | One place; matches existing tab pattern. |
| Concurrency | Multi-job queue, max 1 concurrent file-import job, runs in background when window closed; cancels on app quit. Live capture is exclusive — at most one live session, and starting live blocks new file-import jobs from running until live stops (queued file-import jobs wait) | Predictable resources; usable while a meeting is processing. |
| History | Opt-in save with prompt after each transcription | Privacy-first per "earn trust through transparency". |
| Exports | Markdown + clipboard, plain text, JSON, SRT, VTT | Covers human sharing, integrations, video captions. |
| Module layout | New `src-tauri/src/meeting/` module, Phase 1 files untouched | Low blast radius on existing dictation flow. |
| Audio decoder | `symphonia` + `mp4` crate for mp4 audio track extraction | Pure Rust, no ffmpeg runtime dependency. |
| Phase 1 reuse | `pipeline.rs` calls existing `audio.rs` whisper wrapper directly; adapter inside `pipeline.rs` if needed | Keep dictation regression-proof. |

## 3. Architecture

### 3.1 New module layout

```
src-tauri/src/
├── (existing) audio.rs, paste.rs, ai.rs, hotkey.rs, ...
└── meeting/
    ├── mod.rs          — re-exports, Tauri command registration
    ├── ingest.rs       — file decode (mp3/m4a/wav/ogg/flac/mp4) → PCM; ScreenCaptureKit wrapper
    ├── pipeline.rs     — chunk → transcribe → stitch orchestrator (one job)
    ├── diarize.rs      — sherpa-onnx local + Deepgram client
    ├── llm.rs          — summary + action items via Groq (reuses ai.rs HTTP primitives)
    ├── history.rs      — SQLite + FTS5 (open, write, search, delete, retention)
    ├── jobs.rs         — queue, state machine, progress events
    ├── export.rs       — MD / TXT / JSON / SRT / VTT serialisers
    └── types.rs        — shared types (Transcript, Segment, Job, ExportFormat, etc.)
```

### 3.2 Service boundaries

- `ingest` only converts source (file or live device) → 16kHz mono f32 PCM. No transcription knowledge.
- `pipeline` orchestrates one job from PCM → `Transcript`. Calls `audio.rs` for Whisper, `diarize` for speakers, `llm` for summary. No UI knowledge.
- `jobs` owns the queue and the only Tauri event channel (`meeting://progress`, `meeting://live_segment`, `meeting://done`, `meeting://error`).
- `history` is pure storage; called by `jobs` after the user opts in to save.
- `llm` returns `Option<Summary>`; `None` when no Groq key. Never blocks transcript delivery.
- `export` is a pure function over `Transcript`.

### 3.3 New dependencies

- `symphonia` — audio decode (mp3/m4a/wav/ogg/flac).
- `mp4` — mp4 container demuxing for audio-track extraction.
- `sherpa-rs` — Rust bindings to sherpa-onnx for local speaker diarisation.
- `rusqlite` with `fts5` feature — history storage.
- `objc2-screen-capture-kit` (or hand-rolled `objc2` wrapper) — system audio capture on macOS.
- `cpal` — microphone capture for live mode (already used in Phase 1).

## 4. Data flow

### 4.1 Sub-mode A: file import

```
User drops file
    │
    ▼
ingest.rs ──── mp4? ──► mp4 crate (extract audio track) ──┐
                                                          ▼
                  m4a/mp3/wav/ogg/flac ──► symphonia ──► PCM (16kHz mono f32)
                                                          │
                                                          ▼
                                                  jobs.rs (enqueue Job)
                                                          │
                                                          ▼
                                                  pipeline.rs (when slot free)
                                                          │
                       ┌──────────────────────────────────┤
                       ▼                                  ▼
              chunk PCM (30s windows,           diarize.rs (full PCM)
              2s overlap)                         │
                       │                          ▼
                       ▼                  Segments: [(start, end, speaker)]
              for each chunk:                      │
                  audio.rs Whisper                 │
                  → ChunkResult                    │
                       │                           │
                       ▼                           │
              stitch overlaps                      │
              (dedupe at 2s seam)                  │
                       │                           │
                       └─────────┬─────────────────┘
                                 ▼
                          merge: assign speaker per segment
                                 │
                                 ▼
                          Transcript (segments + speakers)
                                 │
                                 ▼
                       llm.rs (if Groq key) → Summary + ActionItems
                                 │                  │
                                 ▼                  ▼
                          emit `meeting://done` with full result
                                 │
                                 ▼
                          UI prompts "Save to history?"
                                 │
                          ┌──────┴──────┐
                          ▼             ▼
                     history.rs       discard
                     (SQLite)
```

### 4.2 Sub-mode B: live capture

```
User clicks [Live] ──► ScreenCaptureKit (system audio) ──┐
                  ──► cpal/coreaudio (mic, separate)     │
                                                         │
                  Two parallel ring buffers (mic, sys) ◄──┘
                                                         │
                  Every 5s, emit a (mic_chunk, sys_chunk) pair
                                                         │
                                                         ▼
                  pipeline.rs (live mode)
                  ├─ transcribe mic_chunk → "You" segments
                  └─ transcribe sys_chunk → "Other" segments
                                                         │
                                  emit `meeting://live_segment` per result
                                                         │
                                                         ▼
                                  UI appends to rolling transcript
                                                         │
                                  on Stop ────────────────┐
                                                         ▼
                                  diarize.rs on accumulated sys_chunk
                                  (split "Other" → Speaker A/B/C…)
                                                         │
                                                         ▼
                                  llm.rs (if Groq) → Summary + ActionItems
                                                         │
                                                         ▼
                                  same "Save to history?" prompt
```

### 4.3 Key flow choices

- **Live transcription cadence**: 5-second chunks (not 30s). Trades stitching seams for responsiveness.
- **Diarisation timing in live mode**: runs only on Stop. Real-time diarisation is expensive and unstable. Live overlay shows "You" / "Other" only; "Other" splits into multiple speakers after Stop.
- **Stitching**: chunk boundaries dedupe by aligning the trailing ~2s of chunk N with the leading ~2s of chunk N+1 using Whisper word timestamps. Covered by unit tests.
- **PCM normalisation**: everything resampled to 16kHz mono f32 at ingest — Whisper's native rate. No resampling later.
- **Streaming from disk for long files**: resampled PCM is written to `~/Library/Caches/iSpeak/jobs/<jobId>.pcm`; pipeline reads windows from disk to keep memory bounded for multi-hour files. Temp file deleted on completion or cancel.

## 5. Components & state

### 5.1 Job state machine

```
   ┌─────────┐  enqueue   ┌─────────┐  slot free   ┌──────────────┐
   │ created │ ─────────► │ queued  │ ───────────► │ transcribing │
   └─────────┘            └─────────┘              └──────┬───────┘
                              │                           │
                              │ cancel                    │ all chunks done
                              ▼                           ▼
                         ┌──────────┐              ┌────────────┐
                         │ canceled │              │ diarizing  │
                         └──────────┘              └─────┬──────┘
                                                        │
                                                        ▼
                                                ┌──────────────┐
                                                │ summarizing  │ (skipped if no Groq key)
                                                └─────┬────────┘
                                                      │
                                                      ▼
                                              ┌──────────────┐
                                              │ awaiting_    │ (UI shows result + Save? prompt)
                                              │ user_save    │
                                              └─────┬────────┘
                                                    │
                                          ┌─────────┴─────────┐
                                          ▼                   ▼
                                     ┌─────────┐         ┌──────────┐
                                     │  saved  │         │ discarded│
                                     └─────────┘         └──────────┘

   Any state can transition to ┌─────────┐
                               │  error  │  (with reason; partial transcript preserved if any)
                               └─────────┘
```

### 5.2 Core types

```rust
pub enum JobMode {
  FileImport { path: PathBuf },
  LiveCapture,
}

pub enum JobState {
  Created, Queued, Transcribing, Diarizing, Summarizing,
  AwaitingUserSave, Saved, Discarded, Canceled, Error(String),
}

pub struct Job {
  pub id: Uuid,
  pub mode: JobMode,
  pub state: JobState,
  pub created_at: SystemTime,
  pub progress: Progress,  // chunks done / total, or "live" sentinel
}

pub struct Transcript {
  pub id: Uuid,
  pub created_at: SystemTime,
  pub duration_secs: f32,
  pub source: TranscriptSource,  // FileImport(path) | LiveCapture
  pub segments: Vec<Segment>,
  pub summary: Option<String>,   // None if no Groq key
  pub action_items: Vec<String>, // empty if no Groq key
  pub partial: bool,             // true if cancelled mid-run or chunk failures occurred
}

pub struct Segment {
  pub start: f32,  // seconds
  pub end: f32,
  pub speaker: SpeakerLabel,
  pub text: String,
}

pub enum SpeakerLabel {
  You,          // mic track (live mode)
  Other,        // system track before diarisation
  Indexed(u8),  // after diarisation: SpeakerA, SpeakerB, ...
}

pub enum ExportFormat { Markdown, PlainText, Json, Srt, Vtt }
```

### 5.3 Tauri events

| Direction | Event | Payload | When |
|---|---|---|---|
| → backend | `meeting:enqueue_file` | `{ path }` | File dropped or picked |
| → backend | `meeting:start_live` | `{}` | Live button pressed |
| → backend | `meeting:stop_live` | `{ jobId }` | Stop pressed |
| → backend | `meeting:cancel` | `{ jobId }` | Cancel queued or running job |
| → backend | `meeting:save` | `{ jobId, save: bool }` | Response to Save? prompt |
| → backend | `meeting:export` | `{ transcriptId, format, destPath? }` | Export button |
| → backend | `meeting:search_history` | `{ query }` | FTS query |
| → backend | `meeting:delete_history` | `{ transcriptId? }` | Delete one or all |
| ← frontend | `meeting://progress` | `{ jobId, state, progress }` | State transitions + chunk progress |
| ← frontend | `meeting://live_segment` | `{ jobId, segment }` | Per 5s chunk in live mode |
| ← frontend | `meeting://done` | `{ jobId, transcript }` | Job ready, awaiting save |
| ← frontend | `meeting://error` | `{ jobId, reason, partial?: Transcript }` | Failure; partial preserved if any |

### 5.4 SQLite schema

`history.rs` writes the Transcript as JSON, plus pre-computed flat strings (`full_text`, `action_items_text`) so the FTS trigger has direct fields to index — no `json_extract` required.

```sql
CREATE TABLE transcripts (
  id                TEXT PRIMARY KEY,
  created_at        INTEGER NOT NULL,
  duration_secs     REAL NOT NULL,
  source            TEXT NOT NULL,  -- 'file:<path>' or 'live'
  full_text         TEXT NOT NULL,  -- joined segment text, computed on insert
  summary           TEXT,           -- may be NULL when no Groq key
  action_items_text TEXT,           -- action items joined with newlines, may be NULL
  payload           TEXT NOT NULL   -- full Transcript JSON for re-render / export
);

CREATE VIRTUAL TABLE transcripts_fts USING fts5(
  id UNINDEXED,
  full_text,
  summary,
  action_items_text,
  content = 'transcripts',
  content_rowid = 'rowid',
  tokenize = 'porter unicode61'
);

CREATE TRIGGER transcripts_ai AFTER INSERT ON transcripts BEGIN
  INSERT INTO transcripts_fts(rowid, id, full_text, summary, action_items_text)
  VALUES (new.rowid, new.id, new.full_text, new.summary, new.action_items_text);
END;

CREATE TRIGGER transcripts_ad AFTER DELETE ON transcripts BEGIN
  INSERT INTO transcripts_fts(transcripts_fts, rowid, id, full_text, summary, action_items_text)
  VALUES('delete', old.rowid, old.id, old.full_text, old.summary, old.action_items_text);
END;
```

DB path: `~/Library/Application Support/iSpeak/history.db` (Tauri app data dir).

**Audio file retention**: only the transcript is stored. The original audio file (for file-import) is not copied into iSpeak's data dir, and live-capture raw PCM is deleted from the temp cache once the job completes or is discarded.

### 5.5 Settings additions

Extend existing settings with a new section:

```
meeting.diarization        = "off" | "local" | "deepgram"   (default: "local")
meeting.deepgram_api_key   = <secure storage>
meeting.summary_engine     = "off" | "groq"                 (default: "groq" if key set)
meeting.history_retention  = "forever" | "30d" | "90d"      (default: "forever")
```

## 6. UI

### 6.1 Transcribe tab

```
┌─ 🎤 Dictate ── 📄 Transcribe ── ⬇ Models ── ⚙ Settings ─┐
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │   📥  Drop audio or video file here            │    │
│  │       mp3 · m4a · mp4 · wav · ogg · flac       │    │
│  │       or click to browse                       │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
│         [ 🎙 Start live capture ]                       │
│                                                         │
│  ── In progress ──────────────────────────────────────  │
│  meeting-2026-05-16.m4a                                 │
│  ▓▓▓▓▓▓▓▓▓░░░░░░░ chunk 12 / 24 · diarising · 53%       │
│                                              [Cancel]   │
│                                                         │
│  ── History ──────────────────  🔍 Search transcripts…  │
│  · 1:1 with Sam · 2026-05-14 · 28m · 2 speakers   ⋯    │
│  · Standup    · 2026-05-13 · 12m · 3 speakers     ⋯    │
│  · podcast.mp3 · 2026-05-12 · 47m · no speakers   ⋯    │
└─────────────────────────────────────────────────────────┘
```

Clicking a history item opens an inline detail view (same tab, slide-in panel) with: full transcript with speaker chips, summary, action items checklist, export buttons.

### 6.2 Live-capture overlay

Shown while live capture is active:

```
┌── Live transcription · 04:23 · [⏸ Pause]  [⏹ Stop] ─────┐
│                                                          │
│  You       Quick recap of where we are on the auth bug. │
│  Other     We narrowed it down to the session token…    │
│  You       OK. Can you send me the repro steps?         │
│                                                          │
│            (rolling, auto-scrolls to bottom)             │
└──────────────────────────────────────────────────────────┘
```

### 6.3 Settings tab additions

```
┌─ Meeting transcription ────────────────────────────┐
│                                                    │
│  Speaker labels:  [ Off  ◉ Local  Deepgram ]      │
│                                                    │
│  Deepgram API key:  [____________________]  [Set]  │
│  ⚠ Deepgram key required for cloud diarisation     │
│                                                    │
│  Summary + action items:                           │
│    Requires Groq API key (set in Cloud Engine     │
│    section above)                                  │
│    Current status:  ✓ Available  /  ✗ Not set      │
│                                                    │
│  History retention:                                 │
│    [ ◉ Forever  30 days  90 days  ]                │
│    [ Clear all history ]                           │
│                                                    │
│  Local diarisation model:                           │
│    sherpa-onnx pyannote (~100MB)                   │
│    Status:  ⬇ Not downloaded   [Download]          │
│             ✓ Installed                            │
└────────────────────────────────────────────────────┘
```

The diarisation model download mirrors the existing Whisper model download UX (same component, same pattern).

## 7. Error handling and edge cases

### 7.1 Failure matrix

| Failure | Pipeline response | UI surface |
|---|---|---|
| Unsupported file format | Reject at ingest before enqueue | Inline toast on drop zone |
| Corrupt audio mid-stream | Stop pipeline; preserve any completed chunks as partial transcript | `meeting://error` with `partial` payload; UI shows "Partial transcript saved" + retry |
| Whisper chunk fails | Retry chunk once. If still fails, insert `[unintelligible 0:42–1:12]` placeholder and continue | Placeholder visible in transcript |
| sherpa-onnx diarisation fails / model missing | Skip diarisation; label all non-mic audio as `Other` | Inline notice: "Speaker labels unavailable" |
| Deepgram API error (auth, rate, network) | Fall back to local sherpa-onnx | Inline notice: "Deepgram failed, used local diarisation" |
| Groq API error (no key, network, rate) | Skip summary + actions; transcript still delivered | Banner on result: "Summary unavailable — add Groq key in Settings" |
| ScreenCaptureKit permission denied | Block live-capture start; deep-link to System Settings | Modal: "Grant Screen Recording permission" |
| Microphone permission denied | Reuse Phase 1 permission flow | Existing path |
| Disk full when writing history | Surface error, keep transcript in memory until user retries or discards | `meeting://error`; UI keeps result visible |
| Groq Whisper file >25MB | Pre-split before upload | Transparent to user |
| App quit mid-job | Job cancelled, partial PCM discarded, no recovery | Documented behaviour |
| Window closed mid-job | Job continues; reopening window restores state from `jobs.rs` | Standard |
| User cancels running job | Pipeline aborts at next chunk boundary, partial transcript (chunks already done) preserved and surfaced via `meeting://done` with `partial: true`; user still gets the Save? prompt for the partial | Same result UI as a normal job, with a "partial" badge |

### 7.2 Edge cases

- **Very short file (<10s):** skip chunking (single Whisper call), skip diarisation, skip summary.
- **Very long file (>2h):** stream PCM from a temp file on disk rather than holding all in memory; cache cap 5GB with LRU eviction.
- **Silent audio:** Whisper hallucinates on silence ("you", "thanks for watching"). Pre-filter chunks whose audio RMS is below threshold; insert `[silence]` placeholder.
- **ScreenCaptureKit device drop in live mode:** detect via read timeout; auto-resume if device reappears within 5s; otherwise terminate live job with partial.
- **Multiple drops while job running:** queue them. UI shows queue position.
- **History DB corruption:** on open failure, rename to `history.db.corrupt-<timestamp>` and create fresh DB. One-time banner. No silent data loss.

### 7.3 Performance budget

Targets on Apple Silicon, 60-min mono 16kHz audio:

| Stage | Whisper turbo local | Groq Whisper cloud |
|---|---|---|
| Ingest (decode + resample) | <5s | <5s |
| Transcription | 5–8 min | 1–2 min (network bound) |
| Diarisation (sherpa-onnx local) | <30s | <30s |
| Summary (Groq) | <10s | <10s |
| Total wall-clock | <10 min | <4 min |

### 7.4 Threading

- `jobs.rs` runs a single Tokio task that owns the queue and the active worker.
- Heavy CPU work (Whisper, sherpa-onnx, decode) runs on `tokio::task::spawn_blocking`.
- ScreenCaptureKit callback runs on its own thread; pushes samples to a `tokio::sync::mpsc` channel consumed by the live pipeline task.

## 8. Testing

| Layer | What | Where |
|---|---|---|
| Unit | Chunk stitching (overlap dedup correctness) | `pipeline::tests` with synthetic fixtures |
| Unit | Silence-filter threshold (RMS detector) | `pipeline::tests` with generated tones + silence |
| Unit | Speaker-segment merge (Whisper segments × diarisation timestamps) | `pipeline::tests` |
| Unit | Export format serialisation (MD, TXT, JSON, SRT, VTT) | `export::tests` |
| Unit | SQLite + FTS5 round-trip + search | `history::tests` with in-memory DB |
| Integration | Real audio file → transcript end-to-end (decode → chunk → Whisper → MD export, diarisation off) | `tests/file_import.rs` with checked-in 30s wav fixture |
| Integration | Job state machine — queue 3 files, cancel middle, verify states | `tests/jobs.rs` with mocked transcription engine |
| Integration | Partial-on-failure — inject corrupt chunk, verify partial preserved and emitted | `tests/jobs.rs` |
| Manual | Live capture (ScreenCaptureKit needs permission, can't run headless) | Checklist in `docs/phase-3-manual-test.md` |
| Manual | Groq + Deepgram paths (cost money, gate behind env var) | Same checklist |

**Fixtures**: one 30s clean stereo wav (synthetic — two distinct sine tones to simulate two speakers), one short m4a, one mp4 with audio track. Stored in `tests/fixtures/`, checked into git.

## 9. Ship slices

Phase 3 is delivered as four shippable increments. Each ends with green CI and a working build.

| Slice | Includes | Validates |
|---|---|---|
| **3.1 File import MVP** | symphonia + mp4 decode, chunking + stitching, Whisper (local + Groq), job queue, basic Transcribe tab, Markdown + clipboard export | Core pipeline works end-to-end on file import |
| **3.2 Diarisation** | sherpa-onnx local diarisation + Deepgram opt-in, speaker labels in transcript + exports, Settings UI for diarisation | Speakers attributed correctly |
| **3.3 Summary, history, all exports** | Groq summary + action items, SQLite + FTS5, opt-in save prompt, history list + search + delete, retention setting, JSON / TXT / SRT / VTT exports | Complete file-import experience |
| **3.4 Live capture** | ScreenCaptureKit + cpal mic on separate tracks, dual-track live transcription, live overlay UI, deferred diarisation on Stop | Sub-mode B complete |

Phase 3 is **done** when all four slices are merged.

## 10. Open follow-ups (out of scope for Phase 3)

- Resume in-progress jobs across app restarts (currently cancels on quit).
- Real-time diarisation in live mode (currently deferred to Stop).
- Translation alongside transcription (Whisper supports it; Groq Whisper has a translate endpoint).
- Multi-platform live capture (Windows WASAPI loopback — Phase 4+ if Windows port happens).
- Auto-tagging or topic extraction from summaries.
- Ollama-based summarisation with larger local models (>7B). Revisit once a strong small model with 32k+ context becomes available.
