use symphonia::core::meta::{MetadataRevision, StandardTag};

use crate::AudioSpec;

/// Center frequencies of the five peaking bands.
pub const EQ_FREQUENCIES_HZ: [u32; 5] = [60, 230, 910, 3_600, 14_000];
const SMOOTH_MS: u32 = 20;
const SOUND_CHECK_PREAMP_DB: f64 = -6.0;
const LIMITER_KNEE: f64 = 0.9;

/// Tagged ReplayGain adjustments in decibels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayGain {
    /// Normalization gain for this track.
    pub track_db: Option<f64>,
    /// Normalization gain for the whole album.
    pub album_db: Option<f64>,
}

impl ReplayGain {
    pub(crate) fn from_metadata(metadata: Option<&MetadataRevision>, track_id: u32) -> Self {
        let Some(metadata) = metadata else {
            return Self::default();
        };
        let track = metadata
            .per_track
            .iter()
            .find(|track| track.track_id == u64::from(track_id));
        let tags = track
            .into_iter()
            .flat_map(|track| &track.metadata.tags)
            .chain(&metadata.media.tags);
        let track_db = tags.clone().find_map(|tag| match &tag.std {
            Some(StandardTag::ReplayGainTrackGain(value)) => parse_db(value),
            _ => None,
        });
        let album_db = tags.into_iter().find_map(|tag| match &tag.std {
            Some(StandardTag::ReplayGainAlbumGain(value)) => parse_db(value),
            _ => None,
        });
        Self { track_db, album_db }
    }
}

fn parse_db(value: &str) -> Option<f64> {
    let text = value.trim();
    let suffix = text.len().saturating_sub(2);
    let number = if text
        .get(suffix..)
        .is_some_and(|unit| unit.eq_ignore_ascii_case("db"))
    {
        text.get(..suffix)?.trim()
    } else {
        text
    };
    number
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

/// Which ReplayGain value is applied by Sound Check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SoundCheck {
    /// Play without loudness normalization.
    #[default]
    Off,
    /// Normalize each track independently.
    Track,
    /// Use album gain for albums played in order, with a track gain fallback.
    Album,
}

/// Settings for the playback signal chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DspSettings {
    /// Peaking gains in integer dB from -6 through +6, ordered by frequency.
    pub eq_db: [i8; 5],
    /// Linear master volume from 0 to 1.
    pub volume: f32,
    /// Maximum linear master gain from 0 to 1.
    pub volume_limit: f32,
    /// Loudness normalization mode.
    pub sound_check: SoundCheck,
}

impl Default for DspSettings {
    fn default() -> Self {
        Self {
            eq_db: [0; 5],
            volume: 1.0,
            volume_limit: 1.0,
            sound_check: SoundCheck::Off,
        }
    }
}

/// Invalid input to the streaming effects processor.
#[derive(Debug, thiserror::Error)]
pub enum DspError {
    /// Playback settings must be within the supported ranges.
    #[error("EQ gains must be from -6 to +6 dB and volume settings from 0 to 1")]
    Settings,
    /// The output layout is outside the range supported by the processor.
    #[error("Unsupported playback effects layout")]
    Layout,
    /// The buffer contains a partial interleaved frame.
    #[error("PCM buffer does not contain whole audio frames")]
    PartialFrame,
}

/// Processes interleaved PCM on the feeder thread.
///
/// Applies Sound Check, five peaking filters, master volume, and a soft limiter
/// in that order. Filter history is independent per channel.
pub struct Dsp {
    spec: AudioSpec,
    settings: DspSettings,
    replay_gain: ReplayGain,
    filters: Vec<[FilterState; 5]>,
    current: Parameters,
    from: Parameters,
    target: Parameters,
    ramp_frames: u32,
    remaining: u32,
}

#[derive(Clone, Copy, PartialEq)]
struct Parameters {
    eq: [Biquad; 5],
    replay: f64,
    volume: f64,
}

#[derive(Clone, Copy, PartialEq)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

#[derive(Clone, Copy, Default)]
struct FilterState {
    z1: f64,
    z2: f64,
}

impl Dsp {
    /// Initialize the processor with settings for the first track.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported layout or settings outside their ranges.
    pub fn new(
        spec: AudioSpec,
        settings: DspSettings,
        replay_gain: ReplayGain,
    ) -> Result<Self, DspError> {
        if !(8_000..=768_000).contains(&spec.sample_rate) || !(1..=32).contains(&spec.channels) {
            return Err(DspError::Layout);
        }
        validate(settings)?;
        let parameters = Parameters::new(spec.sample_rate, settings, replay_gain);
        Ok(Self {
            spec,
            settings,
            replay_gain,
            filters: vec![[FilterState::default(); 5]; spec.channels],
            current: parameters,
            from: parameters,
            target: parameters,
            ramp_frames: (spec.sample_rate * SMOOTH_MS).div_ceil(1_000),
            remaining: 0,
        })
    }

    /// Change settings with a 20 ms ramp from the last processed frame.
    ///
    /// # Errors
    ///
    /// Returns an error for settings outside their ranges.
    pub fn set_settings(&mut self, settings: DspSettings) -> Result<(), DspError> {
        validate(settings)?;
        if self.settings != settings {
            self.settings = settings;
            self.retarget();
        }
        Ok(())
    }

    /// Set the next track's normalization values.
    pub fn set_replay_gain(&mut self, gain: ReplayGain) {
        if self.replay_gain != gain {
            self.replay_gain = gain;
            self.retarget();
        }
    }

    /// Clear filter history and apply current settings after a discontinuous seek.
    pub fn reset(&mut self) {
        self.filters.fill([FilterState::default(); 5]);
        self.current = self.target;
        self.remaining = 0;
    }

    /// Process complete frames in place. State carries across calls.
    ///
    /// # Errors
    ///
    /// Returns an error for a partial interleaved frame.
    pub fn process(&mut self, samples: &mut [f32]) -> Result<(), DspError> {
        if !samples.len().is_multiple_of(self.spec.channels) {
            return Err(DspError::PartialFrame);
        }
        for frame in samples.chunks_exact_mut(self.spec.channels) {
            if self.remaining != 0 {
                self.remaining -= 1;
                let t = f64::from(self.ramp_frames - self.remaining) / f64::from(self.ramp_frames);
                self.current = self.from.interpolate(self.target, t);
            }
            for (sample, state) in frame.iter_mut().zip(self.filters.iter_mut()) {
                let mut value = if sample.is_finite() {
                    f64::from(*sample)
                } else {
                    0.0
                };
                value *= self.current.replay;
                for (filter, coefficients) in state.iter_mut().zip(self.current.eq) {
                    value = filter.apply(coefficients, value);
                }
                *sample = limit(value * self.current.volume) as f32;
            }
        }
        Ok(())
    }

    fn retarget(&mut self) {
        let target = Parameters::new(self.spec.sample_rate, self.settings, self.replay_gain);
        if target != self.target {
            self.from = self.current;
            self.target = target;
            self.remaining = self.ramp_frames;
        }
    }
}

fn validate(settings: DspSettings) -> Result<(), DspError> {
    if settings.eq_db.iter().all(|gain| (-6..=6).contains(gain))
        && (0.0..=1.0).contains(&settings.volume)
        && (0.0..=1.0).contains(&settings.volume_limit)
    {
        Ok(())
    } else {
        Err(DspError::Settings)
    }
}

impl Parameters {
    fn new(rate: u32, settings: DspSettings, gain: ReplayGain) -> Self {
        let track = gain.track_db.filter(|gain| gain.is_finite());
        let album = gain.album_db.filter(|gain| gain.is_finite());
        let replay = match settings.sound_check {
            SoundCheck::Off => 1.0,
            SoundCheck::Track => normalization(track),
            SoundCheck::Album => normalization(album.or(track)),
        };
        Self {
            eq: std::array::from_fn(|index| {
                Biquad::peak(rate, EQ_FREQUENCIES_HZ[index], settings.eq_db[index])
            }),
            replay,
            volume: f64::from(settings.volume.min(settings.volume_limit)),
        }
    }

    fn interpolate(self, other: Self, t: f64) -> Self {
        Self {
            eq: std::array::from_fn(|index| self.eq[index].interpolate(other.eq[index], t)),
            replay: lerp(self.replay, other.replay, t),
            volume: lerp(self.volume, other.volume, t),
        }
    }
}

fn normalization(gain: Option<f64>) -> f64 {
    10.0_f64.powf((gain.unwrap_or(0.0).clamp(-60.0, 60.0) + SOUND_CHECK_PREAMP_DB) / 20.0)
}

impl Biquad {
    fn peak(rate: u32, frequency: u32, gain: i8) -> Self {
        // W3C Audio EQ Cookbook, peaking EQ with Q = 1.
        let amplitude = 10.0_f64.powf(f64::from(gain) / 40.0);
        if frequency * 2 >= rate {
            return Self {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            };
        }
        let w = std::f64::consts::TAU * f64::from(frequency) / f64::from(rate);
        let (sine, cosine) = w.sin_cos();
        let alpha = sine * 0.5;
        let a0 = 1.0 + alpha / amplitude;
        let a1 = -2.0 * cosine / a0;
        Self {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: a1,
            b2: (1.0 - alpha * amplitude) / a0,
            a1,
            a2: (1.0 - alpha / amplitude) / a0,
        }
    }

    fn interpolate(self, other: Self, t: f64) -> Self {
        Self {
            b0: lerp(self.b0, other.b0, t),
            b1: lerp(self.b1, other.b1, t),
            b2: lerp(self.b2, other.b2, t),
            a1: lerp(self.a1, other.a1, t),
            a2: lerp(self.a2, other.a2, t),
        }
    }
}

impl FilterState {
    fn apply(&mut self, filter: Biquad, input: f64) -> f64 {
        let output = filter.b0 * input + self.z1;
        self.z1 = filter.b1 * input - filter.a1 * output + self.z2;
        self.z2 = filter.b2 * input - filter.a2 * output;
        output
    }
}

fn lerp(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

fn limit(value: f64) -> f64 {
    if value.abs() <= LIMITER_KNEE {
        value
    } else {
        let span = 1.0 - LIMITER_KNEE;
        (LIMITER_KNEE + span * ((value.abs() - LIMITER_KNEE) / span).tanh()).copysign(value)
    }
}
