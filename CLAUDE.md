# iSpeak — Claude Session Handover

## What is iSpeak

A local-first voice dictation app for macOS. Hotkey → record → transcribe (Whisper local or Groq cloud) → optional AI grammar correction → paste text at cursor. Built with Tauri 2 (Rust backend + React/Vite frontend). See SPEC.md for full product spec.

---

## Build & Run

```bash
cd ~/Documents/Personal/Claude/iSpeak
npm install               # first time only
npm run tauri dev         # dev mode
npm run tauri build       # release build (DMG may fail without code signing, .app still works)
```

Install to Applications:
```bash
sudo rm -rf /Applications/iSpeak.app && sudo cp -R src-tauri/target/release/bundle/macos/iSpeak.app /Applications/iSpeak.app
```

Requires: Node 18+, Rust (rustup), cmake (conda install -c conda-forge cmake).

---

## Current Status: Phase 1 + Phase 2 + Phase 2.1 COMPLETE on macOS 26

**Phase 1 (Dictation Core):** App launches, global hotkey works (both push-to-talk and toggle modes), audio capture → transcription → clipboard → paste all functional.

**Phase 2 (AI Post-Processing):** Implemented May 2026. After transcription, optionally runs text through an LLM for grammar/punctuation correction before pasting. Three AI modes:
- **Local (Ollama)** — free, ~300ms extra, configurable model (default `llama3.2:3b`)
- **Cloud Fast** — Groq Llama 3.3 70B (versatile)
- **Cloud Quality** — Groq Llama 3.3 70B (specdec)
- AI failure is non-fatal — always falls back to raw transcription text

**Phase 2.1 (App-Context-Aware Prompts):** Implemented May 2026. Detects the frontmost macOS app via `NSWorkspace` and appends a context hint to the AI system prompt. Chat apps (Slack/Teams/Discord) get casual tone, email apps get formatting-aware corrections, code editors get minimal punctuation, terminals get command-friendly style, Claude gets LLM-prompt-aware prose. Falls back to default grammar correction for unrecognised apps.

**Stability & Quality Fixes (May 14 2026):**
- **Paste "V bug" fix** (`paste.rs`): On slower machines, the Cmd modifier hadn't latched before the V keycode fired, so the chord landed as a literal "v" character in the foreground window. Bumped focus-return delay 150ms → 300ms and added 25ms gaps around the V click. Commit `4e30bd5`.
- **AI prompt v2** (`ai.rs`): Rewrote `BASE_PROMPT` with 7 explicit rules (grammar, ASR error correction with homophone list, disfluency removal, meaning preservation, proper-noun protection, verbatim punctuation, output-format strictness) plus 3 worked examples. Lowered Groq `temperature` to 0.1 for deterministic correction. Ollama path uses the same prompt but its request struct does not yet set a temperature (defaults to ~0.8) — known follow-up if local mode shows wandering output. Commit `fee0a98`.

**UI Overhaul:** Also completed May 2026:
- Custom app icon (lowercase "i" with mic-capsule dot on indigo gradient squircle)
- Status-first Dictate tab with hero area, transcript card with copy button
- 3-tier visual hierarchy (primary/standard/compact settings)
- About tab collapsed to persistent footer
- Empty states and onboarding hints
- Custom dropdown and range slider (no native OS controls)
- Tab icons (mic, download, sparkle)
- Ambient background gradient, hero glow, gradient title bar border
- Improved API key flow with status dots and remove option
- Model selection ("Use" button on installed models)

---

## macOS 26 Fix (applied April 2026)

### Root Cause
macOS 26 (Tahoe) broke `pthread_main_np()` — it returns 0 during `applicationDidFinishLaunching` even on the main thread. This caused `MainThreadMarker::new()` in objc2 to return `None`, cascading panics across tao/wry/muda/rfd. A secondary issue was a corrupt `icon.icns` (8 bytes / zero-size) that threw an NSException during window creation.

Upstream bug: https://github.com/tauri-apps/tao/issues/1171

### The Fix — 2 forked crates
Instead of patching 30+ individual `MainThreadMarker::new()` call sites, we patched the root function in two crates:

1. **objc2 `is_main_thread()`** — hardcoded to return `true` on Apple platforms. Fork: `leetianlee/objc2` branch `macos26-fix`.
2. **tao `util::is_main_thread()`** — same fix. Fork: `leetianlee/tao` branch `macos26-fix`.
3. **Regenerated `icon.icns`** from `icon.png` (was 8 bytes, now proper iconset).

Both forks are referenced via `[patch.crates-io]` in `src-tauri/Cargo.toml`. No vendor directory needed. Cargo pulls the forks automatically during build.

When upstream fixes the macOS 26 issue, remove the `[patch.crates-io]` section and delete the forks.

---

## Architecture Overview

```
src/                          React + Vite + TypeScript frontend
  lib/contract.ts             READ-ONLY interface between frontend and backend
  store/useAppStore.ts        Zustand state
  components/
    SettingsPanel.tsx         Main UI (3 tabs: Dictate, Models, AI)
    ModelDownload.tsx         Whisper model download + selection UI

src-tauri/
  src/
    lib.rs                    Tauri app entry, plugin registration
    commands.rs               All tauri::command handlers
    ai.rs                     AI post-processing (Ollama + Groq) with app-context-aware prompts
    frontmost_app.rs          Frontmost macOS app detection via NSWorkspace
    audio.rs                  cpal mic capture + rubato resampling to 16kHz mono
    whisper_engine.rs         whisper-rs (Metal GPU) local transcription
    groq.rs                   Groq Whisper API (cloud fallback)
    paste.rs                  enigo Cmd+V simulation
    settings.rs               AppSettings struct + store helpers
    error.rs                  AppError with Serialize impl
  tauri.conf.json             Window config (main 620x540, indicator 48x48)
  entitlements.plist          Microphone + accessibility entitlements
  Info.plist                  NSMicrophoneUsageDescription etc.
  capabilities/default.json   Tauri 2 capability permissions

public/
  ispeak.svg                  App favicon (i with mic-capsule dot)
```

## Key Technical Notes

- **Tray icon** in menu bar — click toggles main window visibility (added May 2026)
- **tauri_plugin_global_shortcut** registered in lib.rs with handler for both PTT and toggle modes
- **Audio capture starts immediately** on hotkey press via background `spawn_blocking` task; `execute_stop` signals the stop flag and awaits the handle (fixed April 2026 — previously audio::record was called after stop_flag was set, capturing ~0 audio)
- **AI post-processing** in `execute_stop()` runs between transcription and clipboard write; non-fatal with fallback to raw_text
- **arboard** (not Tauri's clipboard API) is used for clipboard write — Tauri 2's clipboard API has permission issues
- **enigo** simulates Cmd+V to paste — requires Accessibility permission granted by user (release binary needs separate permission grant from dev build)
- Recording state machine lives entirely in Rust (IDLE → RECORDING → PROCESSING → IDLE)
- **Contract simplification (May 2026):** Removed `AIProvider` type, `openai_api_key`, `anthropic_api_key`, `deepgram_api_key` from contract.ts. Cloud AI modes use Groq only (Llama 70B versatile for fast, specdec for quality). Single `groq_api_key` for both transcription and AI post-processing.

---

## Phase Roadmap (from SPEC.md)

- **Phase 1** (DONE): Core dictation — hotkey, record, local/Groq transcription, paste
- **Phase 2** (DONE): AI post-processing via Ollama (local) or Groq (cloud). Universal grammar/punctuation prompt.
- **Phase 2.1** (DONE): App-context-aware prompts — detect frontmost app via NSWorkspace, adjust AI prompt per app category
- **Phase 3**: Meeting transcription mode (long recordings, speaker diarisation, export)
- **Phase 4**: OSS release prep (README, CI, code signing)

## Design Context

- `PRODUCT.md` — product register, users, brand personality, anti-references, design principles
- `DESIGN.md` — visual system ("The Voice Channel"), color tokens, typography, components, do's/don'ts
- `docs/superpowers/specs/2026-05-01-phase2-ai-post-processing-design.md` — Phase 2 design spec
- `docs/superpowers/plans/2026-05-01-phase2-ai-post-processing.md` — Phase 2 implementation plan

## Next Steps

- **AI eval (before Phase 3):** Template at `docs/ai-eval.csv`. Collect 20-30 real dictation samples, run each through Off/Ollama/Groq Fast/Groq Quality, manually judge correction quality, meaning preservation, latency.
- **Phase 3:** Meeting transcription mode
- **Phase 4:** OSS release prep

## Repository

- GitHub: https://github.com/leetianlee/ispeak
- Forked crates for macOS 26 fix: `leetianlee/objc2` and `leetianlee/tao` (branch `macos26-fix` on each)
