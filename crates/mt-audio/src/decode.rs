use std::fs::File;
use std::path::{Path, PathBuf};

use symphonia::core::codecs::audio::{
    AudioCodecParameters, AudioDecoder as SymphoniaDecoder, AudioDecoderOptions,
    well_known::{CODEC_ID_AAC, CODEC_ID_OPUS},
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, probe::Hint};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Duration, Time, TimeBase, Timestamp};

use crate::mp4_timing::{PlaybackWindow, aac_window};

/// Sample rate and channel count of interleaved PCM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    /// Samples per second for each channel.
    pub sample_rate: u32,
    /// Number of audio channels per frame.
    pub channels: usize,
}

/// Failure to open, decode, or seek in a local audio track.
#[derive(Debug, thiserror::Error)]
pub enum AudioDecodeError {
    /// The input could not be opened.
    #[error("Could not open music file {}", path.display())]
    Open {
        /// File that failed to open.
        path: PathBuf,
        /// Filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// No playable audio stream was found.
    #[error("The file contains no playable audio track")]
    NoAudio,
    /// The container or codec rejected the audio.
    #[error("Could not read audio: {0}")]
    Format(#[from] SymphoniaError),
    /// The codec could not decode an Opus packet.
    #[error("Could not decode Opus audio: {0}")]
    Opus(#[from] opus::Error),
    /// The codec supplied an invalid or changing signal layout.
    #[error("Unsupported audio channel count or sample rate")]
    UnsupportedSpec,
    /// The requested time is outside the track.
    #[error("The requested seek time is outside the track")]
    InvalidSeek,
}

enum Codec {
    Symphonia(Box<dyn SymphoniaDecoder>),
    Opus(opus::MSDecoder),
}

/// Streams one audio track as packets of interleaved floating-point PCM.
///
/// Only the current decoded packet is kept in memory. Run decoding and seeking
/// on a feeder thread when playing to a real-time audio device.
pub struct AudioDecoder {
    path: PathBuf,
    exhausted: bool,
    container: Box<dyn FormatReader>,
    codec: Codec,
    track_id: u32,
    spec: AudioSpec,
    time_base: TimeBase,
    start: Timestamp,
    media_start: Timestamp,
    duration_ms: Option<u64>,
    window: Option<PlaybackWindow>,
    seek_target: Option<Timestamp>,
    packet_samples: Vec<f32>,
}

impl AudioDecoder {
    /// Open a local audio file and initialize its default audio track.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened, has no audio, or uses an
    /// unsupported codec or layout.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AudioDecodeError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| AudioDecodeError::Open {
            path: path.to_owned(),
            source,
        })?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        let mut container = symphonia::default::get_probe().probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;
        let track = container
            .tracks()
            .iter()
            .find(|track| {
                track
                    .codec_params
                    .as_ref()
                    .is_some_and(|params| params.is_audio())
            })
            .ok_or(AudioDecodeError::NoAudio)?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or(AudioDecodeError::NoAudio)?;
        let codec = if params.codec == CODEC_ID_OPUS {
            Codec::Opus(opus_decoder(params)?)
        } else {
            Codec::Symphonia(
                symphonia::default::get_codecs()
                    .make_audio_decoder(params, &AudioDecoderOptions::default())?,
            )
        };
        let decoded_params = match &codec {
            Codec::Symphonia(codec) => codec.codec_params(),
            Codec::Opus(_) => params,
        };
        let sample_rate = decoded_params
            .sample_rate
            .or(params.sample_rate)
            .filter(|rate| *rate > 0)
            .ok_or(AudioDecodeError::UnsupportedSpec)?;
        let channels = decoded_params
            .channels
            .as_ref()
            .or(params.channels.as_ref())
            .map(|channels| channels.count())
            .filter(|count| *count > 0)
            .ok_or(AudioDecodeError::UnsupportedSpec)?;
        let spec = AudioSpec {
            sample_rate,
            channels,
        };
        let time_base = track
            .time_base
            .or_else(|| TimeBase::try_from_recip(sample_rate))
            .ok_or(AudioDecodeError::UnsupportedSpec)?;
        let track_id = track.id;
        let audio_start = track.start_ts;
        let duration_ms = track
            .duration
            .and_then(|ticks| time_base.calc_duration(ticks))
            .and_then(|duration| u64::try_from(duration.as_millis()).ok());
        let window = if params.codec == CODEC_ID_AAC {
            File::open(path).ok().and_then(|file| {
                aac_window(file, track_id, sample_rate, container.metadata().current())
            })
        } else {
            None
        };
        let start = window
            .and_then(|window| {
                time_base.calc_timestamp(
                    TimeBase::try_from_recip(sample_rate)?
                        .calc_duration(Duration::from(window.start))?,
                )
            })
            .map_or(audio_start, |offset| {
                Timestamp::new(audio_start.get().saturating_add(offset.get()))
            });
        let duration_ms = window
            .map(|window| (u128::from(window.frames) * 1_000 / u128::from(sample_rate)) as u64)
            .or(duration_ms);
        Ok(Self {
            path: path.to_owned(),
            exhausted: false,
            track_id,
            start,
            media_start: audio_start,
            duration_ms,
            window,
            container,
            codec,
            spec,
            time_base,
            seek_target: None,
            packet_samples: Vec::new(),
        })
    }

    /// Sample rate and channel count used by this track.
    pub fn spec(&self) -> AudioSpec {
        self.spec
    }

    /// Duration indicated by the demuxer, where available.
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// Reposition to a millisecond offset from the start of the track.
    ///
    /// The next packet read discards decoded preroll and returns samples
    /// starting at the requested point.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid offset or a stream that cannot seek.
    pub fn seek(&mut self, position_ms: u64) -> Result<(), AudioDecodeError> {
        if self
            .duration_ms
            .is_some_and(|duration| position_ms > duration)
        {
            return Err(AudioDecodeError::InvalidSeek);
        }
        if self.exhausted {
            // Some containers discard the active media atom at EOF.
            let reopened = Self::open(&self.path)?;
            if reopened.spec != self.spec {
                return Err(AudioDecodeError::UnsupportedSpec);
            }
            *self = reopened;
        }
        let at = self.timestamp(position_ms)?;
        // Recent packets warm up codecs with decoder state such as an MP3
        // bit reservoir or the Opus seek preroll.
        let preroll = if position_ms < 100 {
            self.media_start
        } else {
            self.timestamp(position_ms - 100)?
        };
        self.container.seek(
            SeekMode::Accurate,
            SeekTo::Timestamp {
                ts: preroll,
                track_id: self.track_id,
            },
        )?;
        match &mut self.codec {
            Codec::Symphonia(codec) => codec.reset(),
            Codec::Opus(codec) => codec.reset_state()?,
        }
        self.seek_target = Some(at);
        Ok(())
    }

    fn timestamp(&self, position_ms: u64) -> Result<Timestamp, AudioDecodeError> {
        let relative = self
            .time_base
            .calc_timestamp(Time::from_millis_u64(position_ms))
            .ok_or(AudioDecodeError::InvalidSeek)?;
        let start = self
            .start
            .get()
            .checked_add(relative.get())
            .ok_or(AudioDecodeError::InvalidSeek)?;
        Ok(Timestamp::new(start))
    }

    /// Decode the next nonempty packet and return its interleaved samples.
    ///
    /// The returned slice is borrowed until the next call. `None` means
    /// the track is exhausted.
    ///
    /// # Errors
    ///
    /// Returns an error for an unreadable stream or signal layout change.
    pub fn next_samples(&mut self) -> Result<Option<&[f32]>, AudioDecodeError> {
        if self.exhausted {
            return Ok(None);
        }
        while let Some(packet) = self.container.next_packet()? {
            if packet.track_id != self.track_id {
                continue;
            }
            let mut start = 0;
            let mut end;
            match &mut self.codec {
                Codec::Symphonia(codec) => {
                    let decoded = match codec.decode(&packet) {
                        Ok(decoded) => decoded,
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(error) => return Err(error.into()),
                    };
                    if decoded.spec().rate() != self.spec.sample_rate
                        || decoded.spec().channels().count() != self.spec.channels
                    {
                        return Err(AudioDecodeError::UnsupportedSpec);
                    }
                    decoded.copy_to_vec_interleaved(&mut self.packet_samples);
                    end = self.packet_samples.len();
                    // MP3 and Vorbis apply packet trims in their decoders.
                    // Apply them here only when the buffer still includes them.
                    if (packet.trim_start != Duration::ZERO || packet.trim_end != Duration::ZERO)
                        && end / self.spec.channels
                            > duration_frames(packet.dur, self.time_base, self.spec.sample_rate)
                    {
                        start = duration_frames(
                            packet.trim_start,
                            self.time_base,
                            self.spec.sample_rate,
                        )
                        .saturating_mul(self.spec.channels)
                        .min(end);
                        end = end
                            .saturating_sub(
                                duration_frames(
                                    packet.trim_end,
                                    self.time_base,
                                    self.spec.sample_rate,
                                )
                                .saturating_mul(self.spec.channels),
                            )
                            .max(start);
                    }
                }
                Codec::Opus(codec) => {
                    // An Opus packet contains up to 120 ms at 48 kHz.
                    self.packet_samples.resize(5760 * self.spec.channels, 0.0);
                    let frames =
                        codec.decode_float(&packet.data, &mut self.packet_samples, false)?;
                    start = usize::try_from(packet.trim_start.get())
                        .unwrap_or(usize::MAX)
                        .min(frames)
                        * self.spec.channels;
                    end = frames.saturating_sub(
                        usize::try_from(packet.trim_end.get()).unwrap_or(usize::MAX),
                    ) * self.spec.channels;
                    end = end.max(start);
                }
            }
            if let Some(window) = self.window {
                let packet_frame =
                    i128::from(packet.pts.get()) - i128::from(self.media_start.get());
                let packet_frame = packet_frame
                    * i128::from(self.time_base.numer.get())
                    * i128::from(self.spec.sample_rate)
                    / i128::from(self.time_base.denom.get());
                let requested = self
                    .seek_target
                    .and_then(|target| target.duration_from(self.start))
                    .map_or(0, |ticks| {
                        duration_frames(ticks, self.time_base, self.spec.sample_rate)
                    });
                let first = i128::from(window.start) + requested as i128;
                let last = i128::from(window.start) + i128::from(window.frames);
                let offset = |frame: i128| {
                    usize::try_from((frame - packet_frame).max(0))
                        .unwrap_or(usize::MAX)
                        .saturating_mul(self.spec.channels)
                };
                start = start.max(offset(first)).min(end);
                end = end.min(offset(last));
                if packet_frame >= last {
                    break;
                }
            } else if let Some(target) = self.seek_target
                && let Some(before) = target.duration_from(packet.pts)
            {
                let skip = duration_frames(before, self.time_base, self.spec.sample_rate);
                start = start
                    .saturating_add(skip.saturating_mul(self.spec.channels))
                    .min(end);
            }
            if start < end {
                self.seek_target = None;
                return Ok(Some(&self.packet_samples[start..end]));
            }
        }
        self.exhausted = true;
        Ok(None)
    }
}

fn duration_frames(duration: Duration, time_base: TimeBase, rate: u32) -> usize {
    let frames = u128::from(duration.get()) * u128::from(time_base.numer.get()) * u128::from(rate)
        / u128::from(time_base.denom.get());
    usize::try_from(frames).unwrap_or(usize::MAX)
}

fn opus_decoder(params: &AudioCodecParameters) -> Result<opus::MSDecoder, AudioDecodeError> {
    let header = params
        .extra_data
        .as_deref()
        .filter(|header| header.starts_with(b"OpusHead") && header.len() >= 19)
        .ok_or(AudioDecodeError::UnsupportedSpec)?;
    let channels = usize::from(header[9]);
    let mut codec = match (header[18], channels) {
        (0, 1) => opus::MSDecoder::new(48_000, 1, 0, &[0])?,
        (0, 2) => opus::MSDecoder::new(48_000, 1, 1, &[0, 1])?,
        (1, 1..=8) if header.len() >= 21 + channels => {
            opus::MSDecoder::new(48_000, header[19], header[20], &header[21..21 + channels])?
        }
        _ => return Err(AudioDecodeError::UnsupportedSpec),
    };
    codec.set_gain(i32::from(i16::from_le_bytes([header[16], header[17]])))?;
    Ok(codec)
}
