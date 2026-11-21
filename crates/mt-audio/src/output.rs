use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use crate::sfx::{ClickMixer, ClickRequest};
use crate::{AudioQueue, AudioSpec, Dsp, DspError, DspSettings, RenderError};

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
    /// Playback effects could not be applied.
    #[error(transparent)]
    Dsp(#[from] DspError),
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
    play_queue(&[path.as_ref().to_owned()], position_ms, true)
}

/// Play tracks in order through a single CPAL stream.
///
/// The caller feeds the device until the queue ends. The result counts
/// frames consumed by the device, including the faded portions of transitions.
///
/// # Errors
///
/// Returns an error if a track or the output device fails.
pub fn play_queue(paths: &[PathBuf], position_ms: u64, gapless: bool) -> Result<u64, OutputError> {
    play_queue_with_dsp(paths, position_ms, gapless, DspSettings::default())
}

/// Play a queue through one device with the requested EQ and loudness settings.
///
/// Select album Sound Check when the queue represents an album in track order.
///
/// # Errors
///
/// Returns an error for invalid effects settings, failed media, or device setup.
pub fn play_queue_with_dsp(
    paths: &[PathBuf],
    position_ms: u64,
    gapless: bool,
    settings: DspSettings,
) -> Result<u64, OutputError> {
    let mut output = DeviceOutput::open()?;
    let spec = output.spec;
    let mut queue = AudioQueue::open(paths, spec, position_ms, gapless)?;
    let mut effects = Dsp::new(spec, settings, queue.initial_replay_gain())?;
    let mut processed = Vec::new();

    let mut total = 0;
    let mut started = false;
    let mut watchdog = PlaybackWatchdog::new();
    while let Some(block) = queue.next_block()? {
        effects.set_replay_gain(block.replay_gain);
        processed.clear();
        processed.extend_from_slice(block.samples);
        effects.process(&mut processed)?;
        let mut remaining = processed.as_slice();
        while !remaining.is_empty() {
            let written = output.push(remaining) * spec.channels;
            total += (written / spec.channels) as u64;
            remaining = &remaining[written..];
            if remaining.is_empty() {
                break;
            }
            if !started {
                output.set_playing(true);
                output.start()?;
                started = true;
                watchdog = PlaybackWatchdog::new();
            }
            watchdog.wait(&output.state)?;
        }
    }
    if total == 0 {
        return Ok(0);
    }
    if !started {
        output.set_playing(true);
        output.start()?;
        watchdog = PlaybackWatchdog::new();
    }
    while output.delivered() < total {
        watchdog.wait(&output.state)?;
    }

    // Callback delivery precedes device presentation. Keep the stream alive
    // through the last nonempty buffer's reported presentation time.
    thread::sleep(output.tail());
    Ok(total)
}

pub(crate) struct DeviceOutput {
    stream: cpal::Stream,
    producer: HeapProd<f32>,
    state: Arc<CallbackState>,
    click: Option<ClickRequest>,
    pub(crate) spec: AudioSpec,
}

pub(crate) trait PlaybackOutput {
    fn spec(&self) -> AudioSpec;
    fn push(&mut self, samples: &[f32]) -> usize;
    fn delivered(&self) -> u64;
    fn tail(&self) -> Duration;
    fn failed(&self) -> bool;
    fn set_playing(&self, playing: bool);
    fn flush(&mut self) -> Result<u64, OutputError>;
    fn reopen(&mut self) -> Result<(), OutputError>;
}

impl DeviceOutput {
    pub(crate) fn open() -> Result<Self, OutputError> {
        Self::with_click(None)
    }

    pub(crate) fn with_click(click: Option<ClickRequest>) -> Result<Self, OutputError> {
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
        if !(8_000..=768_000).contains(&spec.sample_rate) || !(1..=32).contains(&spec.channels) {
            return Err(RenderError::InvalidOutput.into());
        }
        let (producer, consumer) = ring(spec).split();
        let state = Arc::new(CallbackState::default());
        let mixer = ClickMixer::new(spec.sample_rate, click.clone());
        let stream = match native_format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::F64 => {
                build_stream::<f64>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::I8 => build_stream::<i8>(&device, config, consumer, state.clone(), mixer),
            SampleFormat::I16 => {
                build_stream::<i16>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::I24 => {
                build_stream::<cpal::I24>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::I32 => {
                build_stream::<i32>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::I64 => {
                build_stream::<i64>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::U8 => build_stream::<u8>(&device, config, consumer, state.clone(), mixer),
            SampleFormat::U16 => {
                build_stream::<u16>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::U24 => {
                build_stream::<cpal::U24>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::U32 => {
                build_stream::<u32>(&device, config, consumer, state.clone(), mixer)
            }
            SampleFormat::U64 => {
                build_stream::<u64>(&device, config, consumer, state.clone(), mixer)
            }
            _ => return Err(OutputError::SampleFormat(native_format)),
        }?;
        Ok(Self {
            stream,
            producer,
            state,
            click,
            spec,
        })
    }

    pub(crate) fn start(&self) -> Result<(), OutputError> {
        self.stream.play()?;
        Ok(())
    }
}

impl PlaybackOutput for DeviceOutput {
    fn spec(&self) -> AudioSpec {
        self.spec
    }

    fn push(&mut self, samples: &[f32]) -> usize {
        self.producer.push_slice(samples) / self.spec.channels
    }

    fn delivered(&self) -> u64 {
        self.state.delivered.load(Ordering::Acquire)
    }

    fn tail(&self) -> Duration {
        Duration::from_nanos(self.state.pending_ns.load(Ordering::Relaxed))
    }

    fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }

    fn set_playing(&self, playing: bool) {
        self.state.playing.store(playing, Ordering::Release);
    }

    fn flush(&mut self) -> Result<u64, OutputError> {
        self.set_playing(false);
        let epoch = self
            .state
            .reset
            .fetch_add(1, Ordering::Release)
            .wrapping_add(1);
        let deadline = Instant::now() + STALL_TIMEOUT;
        while self.state.reset_ack.load(Ordering::Acquire) != epoch {
            if self.failed() || Instant::now() >= deadline {
                return Err(OutputError::Stalled);
            }
            thread::sleep(POLL_INTERVAL);
        }
        Ok(self.delivered())
    }

    fn reopen(&mut self) -> Result<(), OutputError> {
        let next = Self::with_click(self.click.clone())?;
        next.start()?;
        *self = next;
        Ok(())
    }
}

#[derive(Default)]
struct CallbackState {
    playing: AtomicBool,
    reset: AtomicU64,
    reset_ack: AtomicU64,
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
    mut mixer: ClickMixer,
) -> Result<cpal::Stream, cpal::Error> {
    let channels = usize::from(config.channels);
    let rate = u64::from(config.sample_rate);
    let on_error = state.clone();
    let mut reset = 0;
    device.build_output_stream::<T, _, _>(
        config,
        move |output, info| {
            assert_no_alloc::assert_no_alloc(|| {
                let requested = state.reset.load(Ordering::Acquire);
                if requested != reset {
                    consumer.clear();
                    reset = requested;
                    state.pending_ns.store(0, Ordering::Relaxed);
                    state.reset_ack.store(reset, Ordering::Release);
                }
                let music = state.playing.load(Ordering::Acquire);
                let samples = fill_output(&mut consumer, output, channels, music, &mut mixer);
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
    music: bool,
    mixer: &mut ClickMixer,
) -> usize {
    mixer.begin_callback();
    if !music && !mixer.active() {
        output.fill(T::EQUILIBRIUM);
        return 0;
    }

    let mut scratch = [0.0; 256];
    let chunk_size = (scratch.len() / channels) * channels;
    let mut consumed = 0;
    for chunk in output.chunks_mut(chunk_size) {
        let len = chunk.len();
        let count = if music {
            consumer.pop_slice(&mut scratch[..len])
        } else {
            0
        };
        if mixer.active() {
            scratch[count..len].fill(0.0);
            for (frame, pcm) in chunk
                .chunks_mut(channels)
                .zip(scratch[..len].chunks(channels))
            {
                let click = mixer.next_frame();
                // Scale the music to leave headroom for this click sample.
                let headroom = 1.0 - click.abs();
                for (target, &sample) in frame.iter_mut().zip(pcm) {
                    *target = T::from_sample(sample.clamp(-1.0, 1.0) * headroom + click);
                }
            }
        } else {
            for (target, &sample) in chunk[..count].iter_mut().zip(&scratch[..count]) {
                *target = T::from_sample(sample.clamp(-1.0, 1.0));
            }
            chunk[count..].fill(T::EQUILIBRIUM);
        }
        consumed += count;
    }
    consumed
}

#[cfg(test)]
mod tests {
    use ringbuf::traits::Observer;

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
        let consumed = assert_no_alloc::assert_no_alloc(|| {
            fill_output(
                &mut consumer,
                &mut output,
                2,
                true,
                &mut ClickMixer::new(48_000, None),
            )
        });
        assert_eq!(consumed, 4);
        assert_eq!(
            output,
            [
                49_152, 16_384, 40_960, 24_576, 32_768, 32_768, 32_768, 32_768
            ]
        );
    }

    #[test]
    fn click_begins_in_the_next_256_frame_callback_without_consuming_paused_music() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 2,
        };
        let (mut producer, mut consumer) = ring(spec).split();
        let request = Arc::new(AtomicBool::new(false));
        let mut click = ClickMixer::new(spec.sample_rate, Some(request.clone()));
        let mut buffer = [0.0_f32; 512];
        assert_eq!(producer.push_slice(&[0.25; 512]), 512);
        assert_eq!(
            fill_output(&mut consumer, &mut buffer, 2, false, &mut click),
            0
        );
        assert_eq!(buffer, [0.0; 512]);

        request.store(true, Ordering::Release);
        let mut last_nonzero = None;
        for callback in 0..4 {
            let consumed = assert_no_alloc::assert_no_alloc(|| {
                fill_output(&mut consumer, &mut buffer, 2, false, &mut click)
            });
            assert_eq!(consumed, 0);
            assert!(
                buffer
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|frame| frame[0] == frame[1])
            );
            if callback == 0 {
                let first = buffer
                    .iter()
                    .position(|sample| sample.abs() > 1e-6)
                    .unwrap()
                    / 2;
                let worst_case_latency = (256 + first) as f64 / 48_000.0;
                assert!(
                    worst_case_latency < 0.010,
                    "{worst_case_latency:.6} seconds"
                );
            }
            if let Some(last) = buffer.iter().rposition(|sample| sample.abs() > 1e-6) {
                last_nonzero = Some(callback * 256 + last / 2);
            }
        }
        assert!(last_nonzero.unwrap() < 48_000 * 15 / 1_000);
        assert_eq!(consumer.occupied_len(), 512);
    }

    #[test]
    fn click_mixes_with_playing_music_without_clipping_and_then_releases() {
        let spec = AudioSpec {
            sample_rate: 48_000,
            channels: 2,
        };
        let (mut producer, mut consumer) = ring(spec).split();
        let request = Arc::new(AtomicBool::new(true));
        let mut click = ClickMixer::new(spec.sample_rate, Some(request));
        assert_eq!(producer.push_slice(&[0.97; 2048]), 2048);
        let mut result = [0.0_f32; 2048];
        assert_eq!(
            assert_no_alloc::assert_no_alloc(|| fill_output(
                &mut consumer,
                &mut result,
                2,
                true,
                &mut click
            )),
            2048
        );
        assert!(
            result[..512]
                .iter()
                .any(|sample| (sample - 0.97).abs() > 1e-3)
        );
        assert!(result.iter().all(|&sample| (-1.0..=1.0).contains(&sample)));
        assert!(result[1200..].iter().all(|&sample| sample == 0.97));
    }
}
