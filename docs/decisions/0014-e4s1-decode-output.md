# 0014 — E4.S1 Streaming decode and output

- **Status** Accepted (2026-11-19)
- **Story** E4.S1

## Context

The audio spike decoded each track into memory in full. The production
engine needs bounded decoding, accurate seeking, and output at the device's
sample rate.

## Decision

Use Symphonia for demuxing and decoding the supported formats. Decode Ogg
Opus with bundled libopus through the Rust `opus` crate. Keep one decoded
packet and the resampler's working buffers. Seek to a packet before the
requested position, then decode preroll and discard samples before the
target. Reopen the demuxer after end of stream so MP4 seeks have a valid
media atom. [Symphonia seeking](https://docs.rs/symphonia-core/0.6.1/symphonia_core/formats/trait.FormatReader.html#tymethod.seek)

Convert to the device layout and apply Rubato resampling on the feeder.
Flush the converter at end of stream and discard its filter delay. CPAL
pulls interleaved floating-point PCM from a bounded single-producer,
single-consumer ring. The callback converts to the hardware sample format
and fills late data with silence. Debug builds guard the callback with
`assert_no_alloc`. Cargo-deny allows that crate's BSD-1-Clause license
through a version-specific exception. [Rubato resampling](https://docs.rs/rubato/5.1.0/rubato/trait.Resampler.html) [BSD-1-Clause](https://spdx.org/licenses/BSD-1-Clause.html)

## Verification

All 17 valid fixtures decode and seek within 20 ms of the tested target on
Linux. A PCM ramp checks exact frame positioning. Automated checks also
verify resampler tail draining, converted frame counts, native sample
conversion, underrun silence, and the debug allocation guard. All 17
fixtures pass through CPAL on a process-local ALSA null device. CI is
configured to repeat those Linux checks.

The three CPAL device tests passed on an Apple Silicon MacBook Pro
running macOS 26.6.2 through its built-in 48 kHz output. That run covered
all 17 playable fixtures, gapless playback with EQ, and the shared
player. The metadata, decode, gapless and DSP fixture tests also passed
on that Mac.

The CPAL test executable cross-links for 64-bit Windows. For native
execution on a machine with a default audio endpoint, set
`MYTUNES_AUDIO_FIXTURE_DIR` to the folder containing the generated
fixtures and run `device-tests.exe --ignored --test-threads=1`. A native
Windows run and hosted CI remain pending.
