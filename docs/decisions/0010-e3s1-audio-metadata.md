# 0010 — E3.S1 Audio metadata

- **Status** Accepted (2026-11-19)
- **Story** E3.S1

## Context

Folder scanning needs a common result for library tags and stream properties
across the supported audio formats. A missing tag and a malformed audio
container require different handling during an import.

## Decision

Use Lofty 0.25.5 in `mt-meta` with strict parsing and embedded artwork disabled.
Probe the file header first and use the filename extension when the probe is
inconclusive. Inspect MP4 sample entries to distinguish AAC, ALAC, and protected
audio. Use the same reader for files and in-memory test inputs. Convert parser
unwinds to a typed failure at the extraction boundary. Local fuzzing found an
arithmetic panic in Lofty's handling of a malformed Opus stream. [Lofty 0.25.5](https://docs.rs/crate/lofty/0.25.5)

Use the primary tag first and look for missing fields in other tag types. Use the
filename stem when no title is available. Keep missing artist and album values
optional. Store gain in decibels, peak relative to full scale, duration in
milliseconds, sample rate in hertz, and audio bitrate in kilobits per second.
Report bit depth for lossless audio and PCM when available.

## Consequences

The scanner can map successful reads to library rows and report typed failures
for unreadable files, unsupported formats, protected audio, and invalid
headers. Metadata reads validate headers and tags. Playback will also need to
handle decoding errors in audio frames.

Linux CI generates the synthetic fixtures and checks metadata from all eight
formats. The separate nightly workflow seeds the same metadata reader with
those files and runs it for ten minutes under AddressSanitizer.

The fuzz harness allows unwinding so the extraction boundary can convert
dependency panics into errors, matching the application's default Rust panic
strategy. Uncaught panics and AddressSanitizer findings still fail the run.
A fixture regression checks invalid Opus granule positions. A build configured
with `panic = "abort"` cannot recover from dependency panics.

## Verification

Local fixture checks pass for MP3, AAC, ALAC, FLAC, WAV, AIFF, Vorbis and Opus.
Tests also cover filename fallbacks, blank tags, primary tag precedence, invalid
numeric values, DRM detection and malformed Opus granule positions. Workspace
tests and Clippy pass. The local AddressSanitizer run completed 863,830 inputs
in 601 seconds with no escaping panic or sanitizer failure. Hosted CI remains
pending because this checkout has no configured Git remote.
