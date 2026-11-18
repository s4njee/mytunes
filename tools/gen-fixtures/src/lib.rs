//! Synthesis helpers for test fixtures: sine generation and a minimal PCM WAV writer.

/// Samples of a sine wave, interleaved per channel.
#[derive(Debug, Clone)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    /// Interleaved samples in -1.0..=1.0.
    pub samples: Vec<f64>,
}

impl Pcm {
    /// Number of frames (samples per channel).
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// Splits at `frame`, returning `(before, after)`.
    pub fn split_at_frame(&self, frame: usize) -> (Pcm, Pcm) {
        let at = frame * usize::from(self.channels);
        let (a, b) = self.samples.split_at(at.min(self.samples.len()));
        let with = |s: &[f64]| Pcm {
            samples: s.to_vec(),
            ..self.clone()
        };
        (with(a), with(b))
    }
}

/// Generates `seconds` of sine, one frequency per channel, at `amplitude` (linear).
pub fn sine(sample_rate: u32, bits: u16, freqs: &[f64], seconds: f64, amplitude: f64) -> Pcm {
    let frames = (f64::from(sample_rate) * seconds).round() as usize;
    let channels = freqs.len() as u16;
    let mut samples = Vec::with_capacity(frames * freqs.len());
    for n in 0..frames {
        let t = n as f64 / f64::from(sample_rate);
        for f in freqs {
            samples.push(amplitude * (std::f64::consts::TAU * f * t).sin());
        }
    }
    Pcm {
        sample_rate,
        channels,
        bits,
        samples,
    }
}

/// Encodes PCM as a RIFF/WAVE file (16- or 24-bit integer PCM).
pub fn wav_bytes(pcm: &Pcm) -> Vec<u8> {
    assert!(
        pcm.bits == 16 || pcm.bits == 24,
        "only 16/24-bit WAV is supported"
    );
    let bytes_per_sample = usize::from(pcm.bits / 8);
    let data_len = pcm.samples.len() * bytes_per_sample;
    let block_align = pcm.channels * (pcm.bits / 8);
    let byte_rate = pcm.sample_rate * u32::from(block_align);

    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&pcm.channels.to_le_bytes());
    out.extend_from_slice(&pcm.sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&pcm.bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());

    let max = f64::from((1u32 << (pcm.bits - 1)) - 1);
    for s in &pcm.samples {
        let v = (s.clamp(-1.0, 1.0) * max).round() as i32;
        let le = v.to_le_bytes();
        out.extend_from_slice(&le[..bytes_per_sample]);
    }
    out
}

/// Frame at which the gapless pair is split: 3 s plus an offset chosen so the
/// 1 kHz sine is near a peak, not a zero crossing.
pub const GAPLESS_SPLIT_FRAME: usize = 132_311;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_and_length() {
        let pcm = sine(44_100, 16, &[440.0, 660.0], 0.5, 0.25);
        let wav = wav_bytes(&pcm);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 22_050 * 2 * 2);
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            44_100
        );
    }

    #[test]
    fn twenty_four_bit_wav_uses_three_bytes_per_sample() {
        let pcm = sine(96_000, 24, &[1000.0], 0.1, 0.5);
        assert_eq!(wav_bytes(&pcm).len(), 44 + 9_600 * 3);
    }

    #[test]
    fn gapless_split_is_continuous_and_off_zero() {
        let full = sine(44_100, 16, &[1000.0], 6.0, 0.5);
        let split = GAPLESS_SPLIT_FRAME;
        let (a, b) = full.split_at_frame(split);
        assert_eq!(a.frames() + b.frames(), full.frames());
        // The boundary sample is far from a zero crossing, so a gap or click is detectable.
        assert!(
            b.samples[0].abs() > 0.2,
            "boundary sample {} is too close to zero",
            b.samples[0]
        );
        assert_eq!(b.samples[0], full.samples[split]);
    }
}
