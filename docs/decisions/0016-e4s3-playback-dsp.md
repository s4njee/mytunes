# 0016 — E4.S3 Playback DSP

- **Status** Accepted (2026-11-19)
- **Story** E4.S3

## Context

The player needs equalization, volume control, loudness normalization,
and clipping protection on a common signal path across platforms.

## Decision

The feeder applies ReplayGain, five Q=1 peaking filters, master gain,
and a soft limiter, in that order. The filter centers are 60, 230,
910, 3,600, and 14,000 Hz, with integer gain from −6 to +6 dB.
Bands at or above Nyquist are bypassed. Filter history is independent
for each channel. [W3C Audio EQ Cookbook](https://www.w3.org/TR/audio-eq-cookbook/)

The decoder reads track and album ReplayGain from media tags. Queue
boundaries carry the appropriate tags through resampling. Callers
choose album normalization when playing an album in order. Album
normalization falls back to track gain when the album tag is missing.
Both modes use a −6 dB preamp. The limiter has a soft knee at 0.9
and bounds the signal at full scale.

The master target is the lower of volume and the configured limit.
Control changes interpolate filter coefficients and gains over 20 ms.
A discontinuous seek resets filter history. The feeder processes PCM
before it reaches the callback's ring buffer.

## Verification

A stepped sine sweep at 48 kHz matches the five-band response within
±0.5 dB. Tests check the volume limit, the 20 ms ramp across two calls,
track and album normalization, fallback, clipping protection, and
ReplayGain tags across a resampled queue boundary. The Linux CPAL
null-device test runs the gapless pairs with EQ, a volume limit, and
Sound Check applied. Hosted CI is pending.
