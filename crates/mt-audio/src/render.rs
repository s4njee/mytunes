use std::path::Path;

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::{AudioDecodeError, AudioDecoder, AudioSpec};

/// Failure to decode or convert audio for an output device.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
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
        if !(8_000..=768_000).contains(&source.sample_rate)
            || !(8_000..=768_000).contains(&output.sample_rate)
            || !(1..=32).contains(&output.channels)
        {
            return Err(RenderError::InvalidOutput);
        }

        let converter = if source.sample_rate == output.sample_rate {
            None
        } else {
            Some(RateConverter::new(source.sample_rate, output)?)
        };

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
            return converter.next_samples(&mut self.decoder);
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

struct RateConverter {
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

    fn next_samples(&mut self, decoder: &mut AudioDecoder) -> Result<Option<&[f32]>, RenderError> {
        let channels = self.output.channels;
        let source_channels = decoder.spec().channels;
        loop {
            let frames_in = self.fft.input_frames_next();
            let block_samples = frames_in * channels;
            while self.pending.len() < block_samples && !self.exhausted {
                match decoder.next_samples()? {
                    Some(samples) => {
                        self.source_frames += (samples.len() / source_channels) as u64;
                        mix_channels(samples, source_channels, channels, &mut self.pending);
                    }
                    None => self.exhausted = true,
                }
            }

            let remaining = if self.exhausted {
                let total = (u128::from(self.source_frames) * u128::from(self.output.sample_rate))
                    .div_ceil(u128::from(self.source_rate));
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

fn mix_channels(input: &[f32], source: usize, target: usize, out: &mut Vec<f32>) {
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
