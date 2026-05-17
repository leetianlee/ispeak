# Phase 3.2 — Live Meeting Capture (design)

Status:
- **3.2a (mic-only): SHIPPED** — see `src-tauri/src/meeting/live.rs` and
  `src/components/Transcribe/LiveCapture.tsx`.
- **3.2b (ScreenCaptureKit system audio): SCAFFOLDED, NOT WIRED** — deps installed,
  stub at `src-tauri/src/meeting/live_macos.rs` with concrete handoff plan. Needs
  a foreground session to complete (Screen Recording TCC prompt + audio fidelity
  verification cannot be done in a background coding session).

## Goal

Capture audio from an in-progress meeting (Zoom, Teams, Meet, in-person, etc.)
and feed it through the same chunk → Whisper → stitch → summarise pipeline
that file import (3.1) already uses. Sub-mode B from `SPEC.md` Mode 2.

The data contract is already in place:

- `TranscriptSource::LiveCapture` exists.
- `JobMode::LiveCapture` exists.
- The pipeline accepts any `&Path` to a 16k mono PCM file, so a live recorder
  just needs to produce that file (or stream into it) and enqueue the job.

What's missing is the *audio plumbing* on macOS.

## The decision: how do we capture system audio?

macOS does not let an app record system output without a workaround. The three
live options:

| Option | Pros | Cons |
|--------|------|------|
| **A. ScreenCaptureKit (`SCStream` audio-only)** | First-party, no external install, real permissions UX. | Requires macOS 13+ (Screen Recording permission), Swift / Obj-C bridge in Rust, the API is async/delegate-heavy. |
| **B. Virtual audio device (BlackHole / Loopback)** | Easiest to integrate via cpal (just another input device). | User has to install a kext-free driver. Out-of-band setup conflicts with iSpeak's "zero-setup" principle. |
| **C. Mic-only "in-the-room" mode** | Trivial — reuse Phase 1 cpal pipeline. | Misses remote participants on Zoom/Teams calls — the most valuable case. |

We **already** have cpal capture for the mic (Phase 1) and we **already** have
the chunking + transcription pipeline (Phase 3.1). The remaining work is
~80% mechanical wiring once the capture strategy is chosen, and ~20%
ScreenCaptureKit Obj-C interop *if* option A is picked.

## Recommended path

**Ship A + C in one cut**, defer B unless community asks for it.

1. **Phase 3.2a — Mic-only live capture.** Reuse `audio::record` from
   Phase 1. Add `meeting_start_live` / `meeting_stop_live` commands.
   Write the PCM to a temp file as it streams, then on stop, enqueue
   the same file-import pipeline. Ship in days.
2. **Phase 3.2b — System audio via ScreenCaptureKit.** Add an
   `objc2-screen-capture-kit` (or hand-rolled) bridge that opens an
   `SCStream` with audio output enabled, pipes PCM into a second writer,
   and mixes with mic at write time (simple float-sum, then clip).
   Requires Screen Recording permission prompt in `Info.plist` and TCC.

Rationale: 3.2a unblocks "record an in-person meeting and get a
structured summary" — a useful slice on its own — without committing
to the ScreenCaptureKit bridge work. 3.2b layers on top once the basic
flow is proven.

## Interface sketch

```rust
// New Tauri commands
pub fn meeting_start_live(state, source: LiveSource) -> Result<Uuid>
pub fn meeting_stop_live(state, job_id: Uuid) -> Result<()>

pub enum LiveSource {
    MicOnly,
    SystemAudio,       // requires ScreenCaptureKit (3.2b)
    MicAndSystem,      // mix of the two
}
```

Frontend: a "Start live recording" button in the Transcribe tab; a
microphone-level meter to confirm capture is working; a Stop button
that hands off to the existing job/progress UI.

## Open questions

- macOS 13 vs 15 floor for ScreenCaptureKit audio: validate against
  the existing macOS 26 patch set (`leetianlee/objc2`, `leetianlee/tao`).
- Hot-swap mic input mid-recording — out of scope for v1.
- Pause/resume — out of scope for v1.

## Why this is deferred from the current cut

- Real implementation needs hands-on UI verification (mic permissions,
  Screen Recording TCC prompt, audio levels) which a background coding
  session can't perform reliably.
- The capture-strategy choice is a product-level call (zero-setup vs.
  external dependency vs. macOS-version floor) that should be made with
  the maintainer rather than picked unilaterally.

When ready: pick A/B/C above, then proceed with 3.2a → 3.2b → 3.2c
sequencing.
