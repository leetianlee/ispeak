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

## Current Status: Phase 1 + Phase 2 COMPLETE on macOS 26

**Phase 1 (Dictation Core):** App launches, global hotkey works (both push-to-talk and toggle modes), audio capture → transcription → clipboard → paste all functional.

**Phase 2 (AI Post-Processing):** Implemented May 2026. After transcription, optionally runs text through an LLM for grammar/punctuation correction before pasting. Three AI modes:
- **Local (Ollama)** — free, ~300ms extra, configurable model (default `llama3.2:3b`)
- **Cloud Fast** — Claude Haiku via Anthropic API
- **Cloud Quality** — Claude Sonnet via Anthropic API
- AI failure is non-fatal — always falls back to raw transcription text

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

### The Fix — 2 patches instead of 30+
Instead of patching 30+ individual `MainThreadMarker::new()` call sites, we patched the root:

1. **objc2 `is_main_thread()`** — `vendor/objc2/src/main_thread_marker.rs`: hardcoded to return `true` on Apple platforms. Safe because Tauri runs everything on the main thread.
2. **tao `util::is_main_thread()`** — `vendor/tao/src/platform_impl/macos/util/async.rs`: same fix, same rationale.
3. **Regenerated `icon.icns`** from `icon.png` (was 8 bytes, now 5127 bytes with proper iconset).

All previous per-site `unsafe { new_unchecked() }` patches were reverted — the crate code now uses standard `new().unwrap()` / `new().expect()` / `new().ok_or()` which all work because `new()` always returns `Some`.

### Vendor patch maintenance
When modifying vendored crate sources:
```python
import json, hashlib
path = "vendor/<crate>/.cargo-checksum.json"
with open(path) as f: data = json.load(f)
rel = "src/path/to/file.rs"
with open(f"vendor/<crate>/{rel}", "rb") as f:
    data["files"][rel] = hashlib.sha256(f.read()).hexdigest()
with open(path, "w") as f: json.dump(data, f)
```
Then `cargo clean -p <crate>` from `src-tauri/` to force recompile.

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
    ai.rs                     AI post-processing (Ollama + Anthropic clients)
    audio.rs                  cpal mic capture + rubato resampling to 16kHz mono
    whisper_engine.rs         whisper-rs (Metal GPU) local transcription
    groq.rs                   Groq Whisper API (cloud fallback)
    paste.rs                  enigo Cmd+V simulation
    settings.rs               AppSettings struct + store helpers
    error.rs                  AppError with Serialize impl
  vendor/                     Patched crate sources (cargo vendor)
  .cargo/config.toml          Points cargo at vendor/
  tauri.conf.json             Window config (main 620x540, indicator 48x48)
  entitlements.plist          Microphone + accessibility entitlements
  Info.plist                  NSMicrophoneUsageDescription etc.
  capabilities/default.json   Tauri 2 capability permissions

public/
  ispeak.svg                  App favicon (i with mic-capsule dot)
```

## Key Technical Notes

- **No tray icon in tauri.conf.json** (stripped during debugging; re-add as next step)
- **tauri_plugin_global_shortcut** registered in lib.rs with handler for both PTT and toggle modes
- **Audio capture starts immediately** on hotkey press via background `spawn_blocking` task; `execute_stop` signals the stop flag and awaits the handle (fixed April 2026 — previously audio::record was called after stop_flag was set, capturing ~0 audio)
- **AI post-processing** in `execute_stop()` runs between transcription and clipboard write; non-fatal with fallback to raw_text
- **arboard** (not Tauri's clipboard API) is used for clipboard write — Tauri 2's clipboard API has permission issues
- **enigo** simulates Cmd+V to paste — requires Accessibility permission granted by user (release binary needs separate permission grant from dev build)
- Recording state machine lives entirely in Rust (IDLE → RECORDING → PROCESSING → IDLE)
- **Contract simplification (May 2026):** Removed `AIProvider` type and `openai_api_key` from contract.ts. Cloud AI modes use Anthropic only (Haiku for fast, Sonnet for quality). Fixed mapping, no provider selector.

---

## Phase Roadmap (from SPEC.md)

- **Phase 1** (DONE): Core dictation — hotkey, record, local/Groq transcription, paste
- **Phase 2** (DONE): AI post-processing via Ollama (local) or Anthropic (cloud). Universal grammar/punctuation prompt. App-context-aware prompts deferred to Phase 2.1.
- **Phase 3**: Meeting transcription mode (long recordings, speaker diarisation, export)
- **Phase 4**: OSS release prep (README, CI, code signing)

## Design Docs

- `docs/superpowers/specs/2026-05-01-phase2-ai-post-processing-design.md` — Phase 2 design spec
- `docs/superpowers/plans/2026-05-01-phase2-ai-post-processing.md` — Phase 2 implementation plan
