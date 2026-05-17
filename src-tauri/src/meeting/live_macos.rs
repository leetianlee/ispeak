//! Phase 3.2b — macOS system audio capture via ScreenCaptureKit.
//!
//! Captures the system audio mix (everything other apps play) as 48kHz stereo
//! f32 PCM, writes it interleaved to `raw_path`, and returns metadata so the
//! rest of `live.rs` can downmix + resample to the 16k mono that Whisper
//! expects. Video is configured to the minimum legal 2x2 because SCK requires
//! a video size even for audio-only capture; the screen frames are dropped
//! in the delegate.
//!
//! Concurrency: this function is called on a dedicated capture thread in
//! `LiveRecorder::start`. The delegate writes from SCK's sample handler
//! dispatch queue, so the BufWriter is wrapped in `Arc<Mutex<_>>`. The
//! `audio_buffer_list_with_retained_block_buffer` call hands us a
//! `CMBlockBuffer` we must release after copying out the samples.
//!
//! Permission: requires Screen Recording (TCC). The first run will show a
//! system prompt; thereafter the user must approve in System Settings →
//! Privacy & Security → Screen Recording. `NSScreenCaptureUsageDescription`
//! is set in `src-tauri/Info.plist`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_core_audio_types::{AudioBuffer, AudioBufferList};
use objc2_core_media::{CMBlockBuffer, CMSampleBuffer};
use objc2_foundation::{NSArray, NSError, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration,
    SCStreamOutput, SCStreamOutputType, SCWindow,
};

use crate::error::{AppError, Result};
use crate::meeting::live::{write_f32_samples, SourceCaptureMeta};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

pub(crate) fn capture_system_to_file(
    raw_path: &Path,
    stop_flag: Arc<AtomicBool>,
) -> Result<SourceCaptureMeta> {
    // 1. Fetch shareable content. SCK schedules the completion block on an
    //    internal queue, so we use a oneshot mpsc to ferry it back here.
    let (content_tx, content_rx) =
        mpsc::channel::<std::result::Result<Retained<SCShareableContent>, String>>();
    let content_block = RcBlock::new(
        move |content: *mut SCShareableContent, err: *mut NSError| {
            let result = if !content.is_null() {
                match unsafe { Retained::retain(content) } {
                    Some(c) => Ok(c),
                    None => Err("SCShareableContent retain returned nil".to_string()),
                }
            } else if !err.is_null() {
                let msg = unsafe { Retained::retain(err) }
                    .map(|e| format!("{e:?}"))
                    .unwrap_or_else(|| "SCShareableContent error retain failed".into());
                Err(msg)
            } else {
                Err("SCShareableContent returned nil with no error".into())
            };
            let _ = content_tx.send(result);
        },
    );
    unsafe {
        SCShareableContent::getShareableContentWithCompletionHandler(&content_block);
    }
    let content = content_rx
        .recv_timeout(Duration::from_secs(30))
        .map_err(|e| AppError::Meeting(format!("SCShareableContent timeout: {e}")))?
        .map_err(AppError::Meeting)?;

    // 2. Pick first display. SCK insists on a display for audio capture.
    let displays = unsafe { content.displays() };
    if displays.count() == 0 {
        return Err(AppError::Meeting(
            "No displays available for SCK system audio capture".into(),
        ));
    }
    let display = displays.objectAtIndex(0);

    // 3. Content filter — entire display, exclude no windows.
    let empty_windows: Retained<NSArray<SCWindow>> = NSArray::new();
    let filter = unsafe {
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &display,
            &empty_windows,
        )
    };

    // 4. Stream configuration — audio on, minimum video, exclude own audio.
    let cfg = unsafe { SCStreamConfiguration::new() };
    unsafe {
        cfg.setCapturesAudio(true);
        cfg.setSampleRate(SAMPLE_RATE as isize);
        cfg.setChannelCount(CHANNELS as isize);
        cfg.setExcludesCurrentProcessAudio(true);
        cfg.setWidth(2);
        cfg.setHeight(2);
    }

    // 5. Delegate that writes samples to disk.
    let file = std::fs::File::create(raw_path)
        .map_err(|e| AppError::Meeting(format!("create raw system file: {e}")))?;
    let writer = Arc::new(Mutex::new(std::io::BufWriter::new(file)));
    let delegate = AudioDelegate::new(writer.clone(), stop_flag.clone());

    // 6. Stream + dispatch queue for the sample handler.
    let stream = unsafe {
        SCStream::initWithFilter_configuration_delegate(
            SCStream::alloc(),
            &filter,
            &cfg,
            None,
        )
    };
    let queue = DispatchQueue::new("tech.cloudsine.ispeak.sck-audio", None);
    let proto = ProtocolObject::from_ref(&*delegate);
    unsafe {
        stream
            .addStreamOutput_type_sampleHandlerQueue_error(
                proto,
                SCStreamOutputType::Audio,
                Some(&queue),
            )
            .map_err(|e| AppError::Meeting(format!("addStreamOutput failed: {e:?}")))?;
    }

    // 7. Start capture (async → oneshot).
    let (start_tx, start_rx) = mpsc::channel::<std::result::Result<(), String>>();
    let start_block = RcBlock::new(move |err: *mut NSError| {
        let result = if err.is_null() {
            Ok(())
        } else {
            let msg = unsafe { Retained::retain(err) }
                .map(|e| format!("{e:?}"))
                .unwrap_or_else(|| "SCStream start error retain failed".into());
            Err(msg)
        };
        let _ = start_tx.send(result);
    });
    unsafe {
        stream.startCaptureWithCompletionHandler(Some(&start_block));
    }
    start_rx
        .recv_timeout(Duration::from_secs(15))
        .map_err(|e| AppError::Meeting(format!("SCStream start timeout: {e}")))?
        .map_err(|m| AppError::Meeting(format!("SCStream start failed: {m}")))?;

    // 8. Block this thread until stop_flag is set. Sample callbacks fire on
    //    the dispatch queue in the background.
    while !stop_flag.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }

    // 9. Stop capture — wait briefly for the completion handler to drain.
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let stop_block = RcBlock::new(move |_err: *mut NSError| {
        let _ = stop_tx.send(());
    });
    unsafe {
        stream.stopCaptureWithCompletionHandler(Some(&stop_block));
    }
    let _ = stop_rx.recv_timeout(Duration::from_secs(5));

    // 10. Flush the BufWriter. The delegate holds a clone of the Arc; the
    //     dispatch queue is finished by now, so contention is safe.
    {
        use std::io::Write;
        if let Ok(mut w) = writer.lock() {
            let _ = w.flush();
        }
    }

    Ok(SourceCaptureMeta {
        raw_path: raw_path.to_path_buf(),
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    })
}

// ─── SCStreamOutput delegate ────────────────────────────────────────────────

#[derive(Debug)]
struct AudioIvars {
    writer: Arc<Mutex<std::io::BufWriter<std::fs::File>>>,
    stop_flag: Arc<AtomicBool>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[ivars = AudioIvars]
    struct AudioDelegate;

    unsafe impl NSObjectProtocol for AudioDelegate {}

    unsafe impl SCStreamOutput for AudioDelegate {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn stream_did_output_sample_buffer(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            output_type: SCStreamOutputType,
        ) {
            if output_type != SCStreamOutputType::Audio {
                return;
            }
            if self.ivars().stop_flag.load(Ordering::Relaxed) {
                return;
            }
            unsafe { handle_audio_sample(self.ivars(), sample_buffer) };
        }
    }
);

impl AudioDelegate {
    fn new(
        writer: Arc<Mutex<std::io::BufWriter<std::fs::File>>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Retained<Self> {
        let this = Self::alloc().set_ivars(AudioIvars { writer, stop_flag });
        unsafe { msg_send![super(this), init] }
    }
}

/// Pull samples out of the CMSampleBuffer's AudioBufferList and write them
/// interleaved-f32 to disk. Releases the retained block buffer before
/// returning.
///
/// SCK normally hands back planar Float32 (one AudioBuffer per channel) at
/// the rate + channel count we requested, but we handle the interleaved case
/// too just in case the format ever changes.
///
/// # Safety
/// Must be called with a valid audio-type CMSampleBuffer from SCK.
unsafe fn handle_audio_sample(ivars: &AudioIvars, sample_buffer: &CMSampleBuffer) {
    // Stack-allocate space for up to 2 AudioBuffers (we configured channelCount=2).
    // AudioBufferList already has mBuffers[1]; we add one extra AudioBuffer's worth.
    let list_size = std::mem::size_of::<AudioBufferList>() + std::mem::size_of::<AudioBuffer>();
    let mut list_storage = vec![0u8; list_size];
    let list_ptr = list_storage.as_mut_ptr() as *mut AudioBufferList;
    let mut block_buffer: *mut CMBlockBuffer = std::ptr::null_mut();

    let status = unsafe {
        sample_buffer.audio_buffer_list_with_retained_block_buffer(
            std::ptr::null_mut(),
            list_ptr,
            list_size,
            None,
            None,
            0,
            &mut block_buffer,
        )
    };

    // Always release the retained block buffer when we leave this scope.
    let _block_buffer_guard = if !block_buffer.is_null() {
        Some(unsafe { Retained::from_raw(block_buffer) })
    } else {
        None
    };

    if status != 0 {
        return;
    }

    let n_buffers = unsafe { (*list_ptr).mNumberBuffers as usize };
    if n_buffers == 0 {
        return;
    }
    let buffers_ptr = unsafe { (*list_ptr).mBuffers.as_ptr() };

    let interleaved: Vec<f32> = if n_buffers == 1 {
        // Either mono, or stereo with samples already interleaved (mNumberChannels=2).
        let buf = unsafe { &*buffers_ptr };
        if buf.mData.is_null() {
            return;
        }
        let n_floats = (buf.mDataByteSize as usize) / std::mem::size_of::<f32>();
        if n_floats == 0 {
            return;
        }
        let slice = unsafe { std::slice::from_raw_parts(buf.mData as *const f32, n_floats) };
        slice.to_vec()
    } else {
        // Planar — each AudioBuffer holds one channel. Interleave into L R L R ...
        let buffers = unsafe { std::slice::from_raw_parts(buffers_ptr, n_buffers) };
        let channels: Vec<&[f32]> = buffers
            .iter()
            .map(|b| {
                if b.mData.is_null() {
                    &[][..]
                } else {
                    let n = (b.mDataByteSize as usize) / std::mem::size_of::<f32>();
                    unsafe { std::slice::from_raw_parts(b.mData as *const f32, n) }
                }
            })
            .collect();
        let min_len = channels.iter().map(|c| c.len()).min().unwrap_or(0);
        let mut out = Vec::with_capacity(min_len * n_buffers);
        for i in 0..min_len {
            for c in &channels {
                out.push(c[i]);
            }
        }
        out
    };

    if interleaved.is_empty() {
        return;
    }
    if let Ok(mut w) = ivars.writer.lock() {
        let _ = write_f32_samples(&mut *w, &interleaved);
    }
}
