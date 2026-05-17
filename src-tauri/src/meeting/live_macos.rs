//! Phase 3.2b — macOS system audio capture via ScreenCaptureKit.
//!
//! Status: **stubbed**. Dependencies (`objc2-screen-capture-kit`,
//! `objc2-core-media`, `block2`, `dispatch2`) are wired up and the bridging
//! plan is documented in `docs/phase-3.2-live-capture-design.md`. The full
//! `SCStream` + `SCStreamOutput` delegate implementation needs to be done in a
//! foreground session so the Screen Recording TCC permission prompt and the
//! captured-audio fidelity can be verified end-to-end with real Zoom/Teams audio.
//!
//! What still needs wiring (concrete handoff):
//!
//! 1. Use `SCShareableContent::getShareableContentWithCompletionHandler` with a
//!    `block2::RcBlock` and a `std::sync::mpsc` oneshot to receive the
//!    asynchronously-returned content on this thread. Pick the first
//!    `SCDisplay`.
//! 2. Build a content filter:
//!    `SCContentFilter::alloc().initWithDisplay_excludingWindows(&display, &NSArray::new())`.
//! 3. Configure the stream:
//!    - `cfg.setCapturesAudio(true)`
//!    - `cfg.setSampleRate(48_000)`
//!    - `cfg.setChannelCount(2)`
//!    - `cfg.setExcludesCurrentProcessAudio(true)` — keeps our own dictation
//!      audio out of the capture
//!    - `cfg.setWidth(2)`, `cfg.setHeight(2)` — minimum legal values, since
//!      audio-only still requires a valid video size
//! 4. Define a class via `objc2::define_class!` with `#[ivars = Ivars]`
//!    where `Ivars` holds `Arc<Mutex<BufWriter<File>>>` and `Arc<AtomicBool>`.
//!    Implement the `stream:didOutputSampleBuffer:ofType:` selector. In the
//!    body:
//!    - Skip non-audio types (just return on `SCStreamOutputType::Screen` /
//!      `Microphone`).
//!    - Call `sample_buffer.audio_buffer_list_with_retained_block_buffer(...)`
//!      with a stack-allocated `AudioBufferList`.
//!    - Walk the `mBuffers` array, treat each `mData` pointer as `*const f32`
//!      (we asked for LinearPCM Float32 by configuring `sampleRate` and
//!      `channelCount`), and write `mDataByteSize` bytes per buffer.
//! 5. `SCStream::initWithFilter_configuration_delegate(&filter, &cfg, None)`.
//! 6. Create a `dispatch2::Queue` for the sample-handler queue; pass it to
//!    `addStreamOutput_type_sampleHandlerQueue_error(..., SCStreamOutputType::Audio, ...)`.
//! 7. `startCaptureWithCompletionHandler` with an `RcBlock` that signals start
//!    success on a oneshot channel.
//! 8. Spin in the calling thread waiting on `stop_flag`. When set,
//!    `stopCaptureWithCompletionHandler`, join the completion oneshot, flush
//!    the writer, and return `SourceCaptureMeta` with `sample_rate=48_000`,
//!    `channels=2`.
//!
//! The reason for shipping a stub now: getting the delegate ivar lifetime
//! semantics right, threading the Mutex across the dispatch queue, and
//! confirming the actual sample format ScreenCaptureKit hands back are all
//! things that benefit hugely from a live REPL/UI loop. Doing it blind in a
//! background session and shipping broken audio capture would be worse than
//! shipping a clear "not yet" error.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::meeting::live::SourceCaptureMeta;

pub(crate) fn capture_system_to_file(
    _raw_path: &Path,
    _stop_flag: Arc<AtomicBool>,
) -> Result<SourceCaptureMeta> {
    Err(AppError::Meeting(
        "Phase 3.2b (system-audio capture) is not yet wired up. \
         Dependencies are in place; see src-tauri/src/meeting/live_macos.rs \
         for the concrete handoff plan. Use mic-only capture for now."
            .to_string(),
    ))
}
