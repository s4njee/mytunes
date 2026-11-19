use std::path::Path;

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::{AudioDecodeError, AudioDecoder, AudioSpec};

/// Failure to decode or convert audio for an output device.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No tracks were supplied to the queue.
    #[error("The playback queue is empty")]
    EmptyQueue,
    /// The next track's worker could not start.
    #[error("Could not start the playback prefetch worker")]
    PrefetchStart(#[source] std::io::Error),
    /// The next track's worker did not return a result.
    #[error("Could not prepare the next track")]
    PrefetchFailed,
    /// The source file could not be decoded.
    #[error(transparent)]
    Decode(#[from] AudioDecodeError),
    /// The target rate or channel count is invalid.
    #[error("Unsupported audio output layout")]
    InvalidOutput,
    /// The sample-rate converter could not initialize.
    #[error("Could not initialize resampling: {0}")]
    Resampler(#[from] rubato::ResamplerConstructionError),
    /// The sample-rate converter could not process the next block.
    #[error("Could not convert audio samples: {0}")]
    Convert(#[from] rubato::ResampleError),
}

/// Streaming interleaved floating-point samples at the device's rate and layout.
///
/// The feeder uses this type outside the real-time audio callback.
pub struct RenderStream {
    decoder: AudioDecoder,
    output: AudioSpec,
    converter: Option<RateConverter>,
    mixed: Vec<f32>,
}

impl RenderStream {
    /// Open a file and prepare its conversion to the requested output layout.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable media or unsupported conversion settings.
    pub fn open(path: impl AsRef<Path>, output: AudioSpec) -> Result<Self, RenderError> {
        let decoder = AudioDecoder::open(path)?;
        let source = decoder.spec();
        let converter = RateConverter::for_rates(source.sample_rate, output)?;

        Ok(Self {
            decoder,
            output,
            converter,
            mixed: Vec::new(),
        })
    }

    /// Signal layout of the rendered samples.
    pub fn spec(&self) -> AudioSpec {
        self.output
    }

    /// Seek to a position in milliseconds and reset the conversion history.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested position cannot be reached.
    pub fn seek(&mut self, position_ms: u64) -> Result<(), RenderError> {
        self.decoder.seek(position_ms)?;
        if let Some(converter) = &mut self.converter {
            converter.reset();
        }
        Ok(())
    }

    /// Borrow the next nonempty block at the output rate. `None` marks end of file.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding or conversion fails.
    pub fn next_samples(&mut self) -> Result<Option<&[f32]>, RenderError> {
        let source_channels = self.decoder.spec().channels;
        if let Some(converter) = &mut self.converter {
            let decoder = &mut self.decoder;
            let output_channels = self.output.channels;
            return converter.next_samples(|pending, _| {
                let Some(samples) = decoder.next_samples()? else {
                    return Ok(0);
                };
                mix_channels(samples, source_channels, output_channels, pending);
                Ok(samples.len() / source_channels)
            });
        }
        let Some(samples) = self.decoder.next_samples()? else {
            return Ok(None);
        };
        if source_channels == self.output.channels {
            return Ok(Some(samples));
        }
        self.mixed.clear();
        mix_channels(
            samples,
            source_channels,
            self.output.channels,
            &mut self.mixed,
        );
        Ok(Some(&self.mixed))
    }
}

pub(crate) struct RateConverter {
    fft: Fft<f32>,
    output: AudioSpec,
    source_rate: u32,
    pending: Vec<f32>,
    block: Vec<f32>,
    rendered: Vec<f32>,
    source_frames: u64,
    delivered: u64,
    delay: usize,
    exhausted: bool,
}

impl RateConverter {
    pub(crate) fn for_rates(
        source_rate: u32,
        output: AudioSpec,
    ) -> Result<Option<Self>, RenderError> {
        if !(8_000..=768_000).contains(&source_rate)
            || !(8_000..=768_000).contains(&output.sample_rate)
            || !(1..=32).contains(&output.channels)
        {
            return Err(RenderError::InvalidOutput);
        }
        if source_rate == output.sample_rate {
            Ok(None)
        } else {
            Self::new(source_rate, output).map(Some)
        }
    }

    fn new(source_rate: u32, output: AudioSpec) -> Result<Self, RenderError> {
        let fft = Fft::new(
            source_rate as usize,
            output.sample_rate as usize,
            1024,
            output.channels,
            FixedSync::Both,
        )?;
        Ok(Self {
            delay: fft.output_delay(),
            block: vec![0.0; fft.input_frames_max() * output.channels],
            rendered: vec![0.0; fft.output_frames_max() * output.channels],
            fft,
            output,
            source_rate,
            pending: Vec::new(),
            source_frames: 0,
            delivered: 0,
            exhausted: false,
        })
    }

    fn reset(&mut self) {
        self.fft.reset();
        self.pending.clear();
        self.source_frames = 0;
        self.delivered = 0;
        self.delay = self.fft.output_delay();
        self.exhausted = false;
    }

    pub(crate) fn next_samples(
        &mut self,
        mut read: impl FnMut(&mut Vec<f32>, u64) -> Result<usize, RenderError>,
    ) -> Result<Option<&[f32]>, RenderError> {
        let channels = self.output.channels;
        loop {
            let frames_in = self.fft.input_frames_next();
            let block_samples = frames_in * channels;
            while self.pending.len() < block_samples && !self.exhausted {
                let frames = read(&mut self.pending, self.source_frames)?;
                self.source_frames += frames as u64;
                self.exhausted = frames == 0;
            }

            let remaining = if self.exhausted {
                let total = u128::from(output_frame(
                    self.source_frames,
                    self.source_rate,
                    self.output.sample_rate,
                ));
                usize::try_from(total.saturating_sub(u128::from(self.delivered)))
                    .unwrap_or(usize::MAX)
            } else {
                usize::MAX
            };
            if remaining == 0 {
                return Ok(None);
            }

            let count = self.pending.len().min(block_samples);
            self.block[..count].copy_from_slice(&self.pending[..count]);
            self.block[count..block_samples].fill(0.0);
            self.pending.drain(..count);

            let input = InterleavedSlice::new(&self.block, channels, frames_in)
                .map_err(|_| RenderError::InvalidOutput)?;
            let mut output = InterleavedSlice::new_mut(
                &mut self.rendered,
                channels,
                self.fft.output_frames_max(),
            )
            .map_err(|_| RenderError::InvalidOutput)?;
            let (_, frames) = self.fft.process_into_buffer(&input, &mut output, None)?;
            let skip = self.delay.min(frames);
            self.delay -= skip;
            let ready = (frames - skip).min(remaining);
            if ready != 0 {
                self.delivered += ready as u64;
                return Ok(Some(
                    &self.rendered[skip * channels..(skip + ready) * channels],
                ));
            }
        }
    }
}

pub(crate) fn output_frame(source_frame: u64, source_rate: u32, output_rate: u32) -> u64 {
    let scaled =
        (u128::from(source_frame) * u128::from(output_rate)).div_ceil(u128::from(source_rate));
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

pub(crate) fn mix_channels(input: &[f32], source: usize, target: usize, out: &mut Vec<f32>) {
    if source == target {
        out.extend_from_slice(input);
        return;
    }
    for frame in input.chunks_exact(source) {
        if source == 1 {
            out.extend(std::iter::repeat_n(frame[0], target));
        } else if target == 1 {
            out.push(frame.iter().sum::<f32>() / source as f32);
        } else if source < target {
            out.extend_from_slice(frame);
            out.extend(std::iter::repeat_n(0.0, target - source));
        } else {
            let shared = frame[target..].iter().sum::<f32>();
            let divisor = (source - target + 1) as f32;
            out.extend(
                frame[..target]
                    .iter()
                    .map(|sample| (*sample + shared) / divisor),
            );
        }
    }
}
