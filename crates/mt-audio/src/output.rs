use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapRb};

use crate::{AudioSpec, RenderError, RenderStream};

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

const STALL_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(2);

/// Failure while feeding a rendered track to an audio device.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    /// The track could not be rendered for the device.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// There is no default playback device.
    #[error("No default audio output device")]
    NoDevice,
    /// The device's default hardware sample format is unsupported.
    #[error("Unsupported output sample format {0}")]
    SampleFormat(SampleFormat),
    /// CPAL could not open or play the stream.
    #[error("Could not use the audio output: {0}")]
    Device(#[from] cpal::Error),
    /// The audio backend stopped accepting samples.
    #[error("Audio output stopped responding")]
    Stalled,
}

/// Play a local track from an offset through the default CPAL output device.
///
/// This blocks the caller until the device consumes the track's samples.
/// Decoding, channel conversion, and resampling run on the calling feeder
/// thread. The audio callback consumes a bounded lock-free buffer.
///
/// # Errors
///
/// Returns an error for invalid media or seek position, device setup failure,
/// or an output stream that stops consuming samples.
pub fn play_file(path: impl AsRef<Path>, position_ms: u64) -> Result<u64, OutputError> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(OutputError::NoDevice)?;
    let supported = device.default_output_config()?;
    let native_format = supported.sample_format();
    let config = supported.config();
    let spec = AudioSpec {
        sample_rate: config.sample_rate,
        channels: usize::from(config.channels),
    };
    let mut renderer = RenderStream::open(path, spec)?;
    if position_ms != 0 {
        renderer.seek(position_ms)?;
    }

    let (mut producer, consumer) = ring(spec).split();
    let state = Arc::new(CallbackState::default());
    let stream = match native_format {
        SampleFormat::F32 => build_stream::<f32>(&device, config, consumer, state.clone()),
        SampleFormat::F64 => build_stream::<f64>(&device, config, consumer, state.clone()),
        SampleFormat::I8 => build_stream::<i8>(&device, config, consumer, state.clone()),
        SampleFormat::I16 => build_stream::<i16>(&device, config, consumer, state.clone()),
        SampleFormat::I24 => build_stream::<cpal::I24>(&device, config, consumer, state.clone()),
        SampleFormat::I32 => build_stream::<i32>(&device, config, consumer, state.clone()),
        SampleFormat::I64 => build_stream::<i64>(&device, config, consumer, state.clone()),
        SampleFormat::U8 => build_stream::<u8>(&device, config, consumer, state.clone()),
        SampleFormat::U16 => build_stream::<u16>(&device, config, consumer, state.clone()),
        SampleFormat::U24 => build_stream::<cpal::U24>(&device, config, consumer, state.clone()),
        SampleFormat::U32 => build_stream::<u32>(&device, config, consumer, state.clone()),
        SampleFormat::U64 => build_stream::<u64>(&device, config, consumer, state.clone()),
        _ => return Err(OutputError::SampleFormat(native_format)),
    }?;
    let mut total = 0;
    let mut started = false;
    let mut watchdog = PlaybackWatchdog::new();
    while let Some(samples) = renderer.next_samples()? {
        let mut remaining = samples;
        while !remaining.is_empty() {
            let written = producer.push_slice(remaining);
            total += (written / spec.channels) as u64;
            remaining = &remaining[written..];
            if remaining.is_empty() {
                break;
            }
            if !started {
                stream.play()?;
                started = true;
                watchdog = PlaybackWatchdog::new();
            }
            watchdog.wait(&state)?;
        }
    }
    if total == 0 {
        return Ok(0);
    }
    if !started {
        stream.play()?;
        watchdog = PlaybackWatchdog::new();
    }
    while state.delivered.load(Ordering::Acquire) < total {
        watchdog.wait(&state)?;
    }

    // Callback delivery precedes device presentation. Keep the stream alive
    // through the last nonempty buffer's reported presentation time.
    thread::sleep(Duration::from_nanos(
        state.pending_ns.load(Ordering::Relaxed),
    ));
    Ok(total)
}

#[derive(Default)]
struct CallbackState {
    delivered: AtomicU64,
    pending_ns: AtomicU64,
    failed: AtomicBool,
}

struct PlaybackWatchdog {
    last_count: u64,
    last_progress: Instant,
}

impl PlaybackWatchdog {
    fn new() -> Self {
        Self {
            last_count: 0,
            last_progress: Instant::now(),
        }
    }

    fn wait(&mut self, state: &CallbackState) -> Result<(), OutputError> {
        let count = state.delivered.load(Ordering::Acquire);
        if count != self.last_count {
            self.last_count = count;
            self.last_progress = Instant::now();
        }
        if state.failed.load(Ordering::Relaxed) || self.last_progress.elapsed() >= STALL_TIMEOUT {
            return Err(OutputError::Stalled);
        }
        thread::sleep(POLL_INTERVAL);
        Ok(())
    }
}

fn ring(spec: AudioSpec) -> HeapRb<f32> {
    // Bound feeder lookahead to 250 ms at the device rate.
    HeapRb::new(((spec.sample_rate / 4) as usize) * spec.channels)
}

fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut consumer: HeapCons<f32>,
    state: Arc<CallbackState>,
) -> Result<cpal::Stream, cpal::Error> {
    let channels = usize::from(config.channels);
    let rate = u64::from(config.sample_rate);
    let on_error = state.clone();
    device.build_output_stream::<T, _, _>(
        config,
        move |output, info| {
            assert_no_alloc::assert_no_alloc(|| {
                let samples = fill_output(&mut consumer, output, channels);
                if samples != 0 {
                    let frames = (samples / channels) as u64;
                    let timing = info.timestamp();
                    let lead = timing.playback.duration_since(timing.callback);
                    let tail =
                        lead.saturating_add(Duration::from_nanos(frames * 1_000_000_000 / rate));
                    let pending =
                        u64::try_from(tail.min(STALL_TIMEOUT).as_nanos()).unwrap_or(u64::MAX);
                    state.pending_ns.store(pending, Ordering::Relaxed);
                    state.delivered.fetch_add(frames, Ordering::Release);
                }
            });
        },
        move |_| on_error.failed.store(true, Ordering::Relaxed),
        None,
    )
}

fn fill_output<T: Sample + FromSample<f32>>(
    consumer: &mut HeapCons<f32>,
    output: &mut [T],
    channels: usize,
) -> usize {
    let mut scratch = [0.0; 256];
    let chunk_size = (scratch.len() / channels) * channels;
    let mut consumed = 0;
    for chunk in output.chunks_mut(chunk_size) {
        let count = consumer.pop_slice(&mut scratch[..chunk.len()]);
        for (target, &sample) in chunk[..count].iter_mut().zip(&scratch[..count]) {
            *target = T::from_sample(sample.clamp(-1.0, 1.0));
        }
        chunk[count..].fill(T::EQUILIBRIUM);
        consumed += count;
    }
    consumed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_converts_samples_and_fills_underruns_without_allocating() {
        let (mut producer, mut consumer) = ring(AudioSpec {
            sample_rate: 48_000,
            channels: 2,
        })
        .split();
        assert_eq!(producer.push_slice(&[0.5, -0.5, 0.25, -0.25]), 4);
        let mut output = [u16::MAX; 8];
        let consumed =
            assert_no_alloc::assert_no_alloc(|| fill_output(&mut consumer, &mut output, 2));
        assert_eq!(consumed, 4);
        assert_eq!(
            output,
            [
                49_152, 16_384, 40_960, 24_576, 32_768, 32_768, 32_768, 32_768
            ]
        );
    }
}
