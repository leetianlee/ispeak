# iSpeak — Product Specification

> Version: 0.1 — Personal tool / OSS  
> Last updated: 2026-04-26  
> Status: Pre-build

---

## Overview

iSpeak is a local-first, AI-enhanced voice-to-text desktop app for macOS and Windows. It has two modes:

- **Dictate** — real-time voice input pasted directly to the cursor in any app
- **Transcribe** — batch transcription of meeting recordings or audio files with structured output

All processing runs locally by default. Cloud APIs are optional and always require the user's own keys (BYOK). No telemetry, no accounts, no cloud sync. Built for personal productivity and released as open-source under the MIT licence.

---

## Core Principles

| Principle | What it means |
|-----------|---------------|
| **Local-first** | whisper.cpp runs on-device. Ollama handles AI post-processing. No network required for core functionality. |
| **BYOK** | Users supply their own API keys. iSpeak never proxies credentials to any server. |
| **No telemetry** | Zero analytics, crash reporting, or usage tracking of any kind. |
| **Single binary** | `npm run tauri build` produces a self-contained distributable. No Python, no system dependencies. |
| **Opinionated defaults** | Push-to-talk, dark mode, medium Whisper model. Sensible out-of-the-box with settings for power users. |

---

## Mode 1: Dictate

Core use case — user holds a hotkey, speaks, releases, text is transcribed and pasted at the cursor in any focused app.

### Recording State Machine

States are authoritative in Rust. The frontend only reflects state via Tauri events — it never drives transitions.

```
IDLE
  │
  ▼ (hotkey down / toggle press)
RECORDING  ──(< 0.5s audio, discard silently)──▶  IDLE
  │
  ▼ (hotkey up / toggle press)
PROCESSING
  │
  ├──▶ AI post-process (if AI mode ≠ Off)
  │
  ▼ (transcript ready)
  paste to cursor  ──▶  IDLE
```

If an error occurs in any state, emit `app_error` event and return to `IDLE`.

### Hotkey Behaviour

| Setting | Behaviour |
|---------|-----------|
| Push-to-talk (default) | Hold hotkey → record. Release → transcribe + paste. |
| Toggle | Press once → start recording. Press again → transcribe + paste. |

- Default hotkey: `Cmd+Shift+Space` (macOS) / `Ctrl+Shift+Space` (Windows)
- Fully reconfigurable in settings
- If recording is already in progress and a second hotkey is received in toggle mode, stop and process

### Audio Capture

- Use `cpal` Rust crate — **not** the Web Audio API. This avoids sample rate mismatch crashes.
- Capture at the device's native sample rate, resample to **16kHz mono** before passing to Whisper.
- Buffer audio in memory. Maximum recording duration: 60 seconds (configurable, minimum 5s).
- Request microphone permission on first launch via a clear in-app dialog — never rely on the OS surprise prompt.

### Transcription Engines

| Engine | Trigger | Notes |
|--------|---------|-------|
| Local (default) | `whisper-rs` (whisper.cpp Rust bindings) | Metal GPU on Apple Silicon, CUDA on Windows NVIDIA |
| Cloud | Groq Whisper API (`whisper-large-v3-turbo`) | Requires `GROQ_API_KEY` in settings |

**Whisper model download** happens inside the app on first launch. Progress bar shown in settings panel. Models stored in `$APP_DATA/models/`. Default: `ggml-medium.bin` (~1.5 GB).

Available models:

| Model | Size | Speed (M1) | Accuracy |
|-------|------|------------|----------|
| tiny | 75 MB | ~100ms | Low |
| base | 142 MB | ~200ms | Moderate |
| small | 466 MB | ~400ms | Good |
| medium (default) | 1.5 GB | ~900ms | Very good |
| large | 2.9 GB | ~2000ms | Best |

### AI Post-Processing

Applied after transcription, before paste. Optional. Off by default — user must explicitly enable.

| AI Mode | Engine | Latency | Cost |
|---------|--------|---------|------|
| Off | — | 0ms | Free |
| Local | Ollama (`phi3.5` or `llama3.2:3b`) | ~300ms | Free |
| Cloud Fast | Claude Haiku / GPT-4o-mini | ~400ms | ~$0.001/call |
| Cloud Quality | Claude Sonnet | ~800ms | ~$0.005/call |

**Prompt strategy** — system prompt is app-context aware. iSpeak reads the frontmost app name (Accessibility API on macOS, `GetForegroundWindow` on Windows) and selects a prompt variant:

| Active App | Prompt Behaviour |
|------------|-----------------|
| Slack / Teams / Discord | Casual tone, no formal punctuation, preserve emoji intent |
| Mail / Outlook / Gmail | Email formatting, aware of greeting and sign-off structure |
| VS Code / Cursor / Zed | Minimal punctuation, code-comment friendly, no auto-capitalisation |
| Notion / Obsidian | Clean prose, preserve bullet intent |
| Default | Correct grammar and punctuation only |

Rules:
- System prompt must be **< 200 tokens**
- Response must be **corrected text only** — no explanation, no wrapper
- Enable **Anthropic prompt caching** on the system prompt prefix to minimise repeated costs

### UI Components

**Menubar / system tray icon**
- iSpeak lives in the menubar (macOS) or system tray (Windows)
- Left-click: open settings panel
- Right-click: quick menu (toggle recording mode, mute microphone, quit)

**Floating recording indicator**
- Small overlay, top-right of screen by default
- `IDLE` → hidden (or minimal dot)
- `RECORDING` → red pulsing circle with mic icon
- `PROCESSING` → amber spinner
- Draggable to any screen position. Position persisted to settings.
- Transparent background, rounded, no title bar

**Settings panel**
- Microphone selector (lists available devices, marks default)
- Hotkey configurator
- Recording mode: push-to-talk / toggle
- Whisper model selector + download button with progress bar
- AI mode selector + API key fields (masked input, stored via `tauri-plugin-store`)
- Ollama base URL (default: `http://localhost:11434`)
- Max recording duration slider

---

## Mode 2: Transcribe (Meeting)

> **Phase 3 — not in v1.** Core architecture must not block this from being added later.

### Sub-mode A: File Import (ship first)

1. User drags an audio/video file onto the app window, or uses a file picker.
2. Accepted formats: `.mp4`, `.m4a`, `.mp3`, `.wav`, `.ogg`
3. Audio is chunked: 30-second windows with 2-second overlap. Stitched by timestamp.
4. Optional speaker diarization via Deepgram API (`DEEPGRAM_API_KEY`). If no key, speaker labels are omitted.
5. LLM post-processing generates: 200-word summary + bullet-point action items list.
6. Output displayed in app. Export to Markdown file or copy to clipboard.
7. Transcript saved to SQLite history with full-text search.

### Sub-mode B: Live Meeting Capture (ship after A is stable)

- Captures system audio via **BlackHole** virtual audio driver (macOS) or WASAPI loopback (Windows).
- BlackHole must be installed separately by the user. README must document this clearly.
- Real-time rolling transcript displayed as recording proceeds.
- Same post-processing pipeline as file import on stop.

---

## Tech Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| App shell | Tauri 2.x | <15 MB bundle, Rust security model, cross-platform |
| Frontend | React 18 + Vite + TypeScript | Fast HMR, type safety, large ecosystem |
| Styling | Tailwind CSS | Utility-first, no CSS conflicts between components |
| State | Zustand | Lightweight, no boilerplate, works well with Tauri events |
| Audio capture | `cpal` (Rust) | Native audio I/O, handles sample rate negotiation |
| Transcription | `whisper-rs` | Rust bindings for whisper.cpp, Metal + CUDA support |
| AI — local | Ollama HTTP API | Free inference, user controls model |
| AI — cloud | Anthropic / OpenAI / Groq APIs | BYOK, user chooses provider |
| Storage | SQLite via `tauri-plugin-sql` | Local, fast, no server |
| Settings persistence | `tauri-plugin-store` | JSON key-value store for config |
| Global hotkeys | `tauri-plugin-global-shortcut` | Cross-platform, works outside app focus |

---

## Interface Contract

The Tauri command and event interface is defined in `src/lib/contract.ts`.

**This file is read-only for all contributors and AI coding agents.** It may only be modified by an explicit architectural decision documented in a GitHub issue or PR discussion. The Rust backend implements these commands. The frontend calls them. Neither side deviates.

See `src/lib/contract.ts` for the full type definitions.

---

## Out of Scope

The following will not be built. PRs for these will not be accepted until the core is stable across both modes.

- User accounts or authentication
- Cloud sync, backup, or remote storage
- Payments or Stripe integration
- Multi-user or team features
- Browser extension
- Mobile (iOS / Android)
- Custom Whisper model fine-tuning
- Real-time streaming transcription (word-by-word display during recording)
- Mac App Store or Windows Store distribution (v1)
- Windows support in v1 (macOS arm64 first, Windows added in Phase 4)

---

## Known Mistakes to Avoid

Sourced from prior research on similar projects (notably Albert Olgaard's "Typer" build).

1. **Never use Web Audio API for mic capture.** Use `cpal`. The browser audio stack requests 48kHz; many mics only support 44.1kHz or 16kHz natively. This causes a class of silent failures and crashes that are hard to debug. `cpal` auto-negotiates sample rate.

2. **Never use Python `whisper`.** Use `whisper-rs` (whisper.cpp Rust bindings). Python whisper is 5–10× slower and requires a Python runtime as a dependency.

3. **Recording state lives in Rust, nowhere else.** No implicit boolean flags in JavaScript. All state transitions go through the Rust state machine and are broadcast to the frontend via Tauri events. Frontend is a view.

4. **No parallel AI agents working on overlapping files.** If using AI coding assistants: one agent owns `src-tauri/`, one owns `src/`. Neither touches the other's directory. The contract file is the only shared surface.

5. **No hardcoded or committed API keys.** All credentials stored via `tauri-plugin-store`. Never in source, never in `.env` files committed to the repo.

6. **Set `data-tauri-drag-region` from day one.** Not as a post-launch fix. Apply it to the settings panel header and the floating indicator.

7. **Request permissions explicitly on first launch** with in-app UI that explains why. Microphone permission and Accessibility permission (for frontmost app detection) both need clear explanatory dialogs before the system prompt fires.

8. **Clear Claude context when it goes in circles.** If an AI coding session has been debugging the same issue for more than 3 attempts without progress, start a fresh session with a focused, minimal reproduction of the problem. Do not keep feeding it the same logs.

---

## Build Phases

| Phase | Scope | Exit Criteria |
|-------|-------|---------------|
| **1 — Dictation Core** | cpal audio, whisper-rs, global hotkey, push-to-talk + toggle, paste to cursor, floating indicator, settings panel | Can dictate into VS Code and Slack with local Whisper model |
| **2 — AI Layer** | Ollama integration, app context detection, context-aware prompts, cloud API options, prompt caching, custom vocabulary | AI-enhanced paste works with Ollama locally and Groq cloud, mode togglable in settings |
| **3 — Meeting Mode** | Audio file import, chunking, optional diarization, summary + action items, Markdown export, history + search | Can drop a Zoom `.m4a`, get a structured transcript with summary |
| **4 — OSS Release** | GitHub Actions CI, arm64 + x86_64 binaries, Windows build, README, BYOK setup guide, auto-update | A developer can clone the repo, run one command, and have a working app |

---

## Repository Structure (target)

```
ispeak/
├── src/                        # React frontend
│   ├── lib/
│   │   └── contract.ts         # READ-ONLY interface contract
│   ├── components/
│   │   ├── SettingsPanel.tsx
│   │   ├── RecordingIndicator.tsx
│   │   └── MeetingView.tsx
│   ├── store/                  # Zustand state
│   └── main.tsx
├── src-tauri/                  # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands.rs         # Implements all commands in contract.ts
│   │   ├── audio.rs            # cpal audio capture
│   │   ├── whisper.rs          # whisper-rs integration
│   │   ├── ai.rs               # Ollama + cloud AI post-processing
│   │   └── storage.rs          # SQLite via tauri-plugin-sql
│   └── Cargo.toml
├── SPEC.md                     # This file
└── README.md
```
