use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use crate::render::{RateConverter, mix_channels, output_frame};
use crate::{AudioDecoder, AudioSpec, RenderError, ReplayGain};

const PREFETCH_AHEAD_MS: u64 = 10_500;
const FADE_MS: usize = 5;

/// PCM from one track in a rendered queue.
pub struct QueuedBlock<'a> {
    /// Zero-based index in the original queue.
    pub track_index: usize,
    /// Loudness normalization tags for the track supplying the block.
    pub replay_gain: ReplayGain,
    /// Interleaved samples in the queue's output layout.
    pub samples: &'a [f32],
}

/// A streaming queue with bounded prefetch and sample-contiguous boundaries.
pub struct AudioQueue {
    input: QueueInput,
    run: OutputRun,
    fade_frames: usize,
    tail: Vec<f32>,
    faded: Vec<f32>,
    ended: bool,
}

struct OutputRun {
    rate: u32,
    converter: Option<RateConverter>,
    markers: VecDeque<TrackMarker>,
    track_index: usize,
    replay_gain: ReplayGain,
    written: u64,
    total_input: u64,
    samples: Vec<f32>,
    sample_offset: usize,
}

struct TrackMarker {
    frame: u64,
    index: usize,
    replay_gain: ReplayGain,
}

struct QueueInput {
    output: AudioSpec,
    current: PreparedTrack,
    paths: VecDeque<PathBuf>,
    prefetch: Option<JoinHandle<Result<PreparedTrack, RenderError>>>,
    index: usize,
    position_ms: u64,
    frames_seen: u64,
    exhausted: bool,
}

struct PreparedTrack {
    decoder: AudioDecoder,
    head: Option<Vec<f32>>,
}

impl PreparedTrack {
    fn open(path: &Path, position_ms: u64) -> Result<Self, RenderError> {
        let mut decoder = AudioDecoder::open(path)?;
        if position_ms != 0 {
            decoder.seek(position_ms)?;
        }
        let head = decoder.next_samples()?.map(<[f32]>::to_vec);
        Ok(Self { decoder, head })
    }

    fn append(&mut self, channels: usize, output: &mut Vec<f32>) -> Result<usize, RenderError> {
        let source_channels = self.decoder.spec().channels;
        let head = self.head.take();
        let block = match &head {
            Some(head) => Some(head.as_slice()),
            None => self.decoder.next_samples()?,
        };
        if let Some(samples) = block {
            mix_channels(samples, source_channels, channels, output);
            Ok(samples.len() / source_channels)
        } else {
            Ok(0)
        }
    }
}

impl AudioQueue {
    /// Open a playlist at the requested millisecond offset in its first track.
    ///
    /// With gapless enabled, tracks follow directly. Otherwise the last and
    /// first 5 ms at a transition fade to and from zero with no inserted frames.
    ///
    /// # Errors
    ///
    /// Returns an error if the queue is empty or its first track cannot render.
    pub fn open(
        paths: &[PathBuf],
        output: AudioSpec,
        position_ms: u64,
        gapless: bool,
    ) -> Result<Self, RenderError> {
        let (first, rest) = paths.split_first().ok_or(RenderError::EmptyQueue)?;
        let current = PreparedTrack::open(first, position_ms)?;
        let run = OutputRun::new(
            current.decoder.spec().sample_rate,
            output,
            0,
            current.decoder.replay_gain(),
        )?;
        let fade_frames = if gapless {
            0
        } else {
            ((output.sample_rate as usize * FADE_MS).div_ceil(1000)).max(2)
        };
        Ok(Self {
            input: QueueInput {
                output,
                current,
                paths: rest.iter().cloned().collect(),
                prefetch: None,
                index: 0,
                position_ms,
                frames_seen: 0,
                exhausted: false,
            },
            run,
            fade_frames,
            tail: Vec::new(),
            faded: Vec::new(),
            ended: false,
        })
    }

    pub(crate) fn initial_replay_gain(&self) -> ReplayGain {
        self.run.replay_gain
    }

    /// Fetch the next rendered block or end of the queue.
    ///
    /// The following track is prepared on a worker roughly ten seconds before
    /// its boundary, or immediately when duration is unknown.
    ///
    /// # Errors
    ///
    /// Returns an error if a track cannot be rendered.
    pub fn next_block(&mut self) -> Result<Option<QueuedBlock<'_>>, RenderError> {
        if self.ended {
            return Ok(None);
        }
        let continuous = self.fade_frames == 0;
        let channels = self.input.output.channels;
        loop {
            if self.run.sample_offset == self.run.samples.len() {
                let read = self.run.fill(&mut self.input, continuous)?;
                if !read {
                    if !self.tail.is_empty() {
                        self.faded.clear();
                        std::mem::swap(&mut self.faded, &mut self.tail);
                        let last = (self.faded.len() / channels).saturating_sub(1);
                        let scale = last.max(1) as f32;
                        fade(&mut self.faded, channels, |frame| {
                            (last - frame) as f32 / scale
                        });
                        return Ok(Some(QueuedBlock {
                            track_index: self.run.track_index,
                            replay_gain: self.run.replay_gain,
                            samples: &self.faded,
                        }));
                    }
                    if self.input.exhausted || (!continuous && !self.input.advance()?) {
                        self.ended = true;
                        return Ok(None);
                    }
                    self.run = OutputRun::new(
                        self.input.current.decoder.spec().sample_rate,
                        self.input.output,
                        self.input.index,
                        self.input.current.decoder.replay_gain(),
                    )?;
                    continue;
                }
            }

            let track_frame = self.run.written;
            let (start, end) = self.run.take_segment(channels);
            if continuous {
                return Ok(Some(QueuedBlock {
                    track_index: self.run.track_index,
                    replay_gain: self.run.replay_gain,
                    samples: &self.run.samples[start..end],
                }));
            }
            self.faded.clear();
            std::mem::swap(&mut self.faded, &mut self.tail);
            let existing = self.faded.len();
            self.faded.extend_from_slice(&self.run.samples[start..end]);
            if self.run.track_index != 0 {
                let elapsed = usize::try_from(track_frame).unwrap_or(usize::MAX);
                fade(&mut self.faded[existing..], channels, |frame| {
                    let at = frame.saturating_add(elapsed).min(self.fade_frames - 1);
                    at as f32 / (self.fade_frames - 1) as f32
                });
            }
            if self.input.has_next() {
                let split = self.faded.len().saturating_sub(self.fade_frames * channels);
                self.tail.extend_from_slice(&self.faded[split..]);
                self.faded.truncate(split);
            }
            if !self.faded.is_empty() {
                return Ok(Some(QueuedBlock {
                    track_index: self.run.track_index,
                    replay_gain: self.run.replay_gain,
                    samples: &self.faded,
                }));
            }
        }
    }
}

impl OutputRun {
    fn new(
        rate: u32,
        output: AudioSpec,
        track_index: usize,
        replay_gain: ReplayGain,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            rate,
            converter: RateConverter::for_rates(rate, output)?,
            markers: VecDeque::new(),
            track_index,
            replay_gain,
            written: 0,
            total_input: 0,
            samples: Vec::new(),
            sample_offset: 0,
        })
    }

    fn fill(&mut self, input: &mut QueueInput, continuous: bool) -> Result<bool, RenderError> {
        self.samples.clear();
        self.sample_offset = 0;
        let mut read = |pending: &mut Vec<f32>, count: u64| {
            input.append(pending, self.rate, count, continuous, &mut self.markers)
        };
        if let Some(converter) = &mut self.converter {
            if let Some(samples) = converter.next_samples(read)? {
                self.samples.extend_from_slice(samples);
            }
        } else {
            self.total_input += read(&mut self.samples, self.total_input)? as u64;
        }
        Ok(!self.samples.is_empty())
    }

    fn take_segment(&mut self, channels: usize) -> (usize, usize) {
        while self
            .markers
            .front()
            .is_some_and(|marker| marker.frame <= self.written)
        {
            if let Some(marker) = self.markers.pop_front() {
                self.track_index = marker.index;
                self.replay_gain = marker.replay_gain;
            }
        }
        let start = self.sample_offset;
        let available = (self.samples.len() - start) / channels;
        let frames = self.markers.front().map_or(available, |marker| {
            usize::try_from(marker.frame - self.written)
                .unwrap_or(usize::MAX)
                .min(available)
        });
        self.written += frames as u64;
        self.sample_offset += frames * channels;
        (start, self.sample_offset)
    }
}

impl QueueInput {
    fn append(
        &mut self,
        pending: &mut Vec<f32>,
        run_rate: u32,
        run_frames: u64,
        continuous: bool,
        markers: &mut VecDeque<TrackMarker>,
    ) -> Result<usize, RenderError> {
        while !self.exhausted && self.current.decoder.spec().sample_rate == run_rate {
            self.prefetch_if_due()?;
            let frames = self.current.append(self.output.channels, pending)?;
            if frames != 0 {
                self.frames_seen += frames as u64;
                return Ok(frames);
            }
            if !continuous || !self.advance()? {
                break;
            }
            if self.current.decoder.spec().sample_rate == run_rate {
                markers.push_back(TrackMarker {
                    frame: output_frame(run_frames, run_rate, self.output.sample_rate),
                    index: self.index,
                    replay_gain: self.current.decoder.replay_gain(),
                });
            }
        }
        Ok(0)
    }

    fn has_next(&self) -> bool {
        self.prefetch.is_some() || !self.paths.is_empty()
    }

    fn prefetch_if_due(&mut self) -> Result<(), RenderError> {
        if self.prefetch.is_some() || self.paths.is_empty() {
            return Ok(());
        }
        let rate = self.current.decoder.spec().sample_rate;
        let elapsed = self
            .position_ms
            .saturating_add(self.frames_seen.saturating_mul(1_000) / u64::from(rate));
        if self
            .current
            .decoder
            .duration_ms()
            .is_some_and(|length| length.saturating_sub(elapsed) > PREFETCH_AHEAD_MS)
        {
            return Ok(());
        }
        if let Some(path) = self.paths.pop_front() {
            self.prefetch = Some(
                thread::Builder::new()
                    .name("mt-audio-prefetch".into())
                    .spawn(move || PreparedTrack::open(&path, 0))
                    .map_err(RenderError::PrefetchStart)?,
            );
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<bool, RenderError> {
        self.current = if let Some(worker) = self.prefetch.take() {
            worker.join().map_err(|_| RenderError::PrefetchFailed)??
        } else if let Some(path) = self.paths.pop_front() {
            PreparedTrack::open(&path, 0)?
        } else {
            self.exhausted = true;
            return Ok(false);
        };
        self.index += 1;
        self.position_ms = 0;
        self.frames_seen = 0;
        Ok(true)
    }
}

fn fade(samples: &mut [f32], channels: usize, gain: impl Fn(usize) -> f32) {
    for (index, frame) in samples.chunks_exact_mut(channels).enumerate() {
        let gain = gain(index);
        for sample in frame {
            *sample *= gain;
        }
    }
}
