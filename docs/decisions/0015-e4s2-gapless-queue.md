# 0015 — E4.S2 Gapless queue

- **Status** Accepted (2026-11-19)
- **Story** E4.S2

## Context

An album split into separate files needs contiguous playback through one
device stream. Encoder priming and padding must be excluded even when
the codec produces PCM for those frames.

## Decision

Prime the next track on a worker around ten seconds before its boundary,
or immediately for short tracks and tracks with unknown length. Feed the
queue through one CPAL stream. Keep one Rubato resampler across adjacent
tracks with the same source rate. A source rate change flushes the current
resampler and starts a new conversion. Track markers are mapped to device
frames from the cumulative input timeline.

Use Symphonia's MP3 packet trimming for LAME delay and padding. For AAC,
prefer the tagged `iTunSMPB` presentation window and fall back to the
MP4 edit list and media duration. Read the movie box directly to avoid
linear reads through the audio payload for edit-list lookup. A coarse
movie timescale may round the edit length. The media timeline supplies
the exact length when the two differ only by that rounding. [MP4 timing fields](https://docs.rs/mp4parse/0.17.0/mp4parse/struct.Track.html)

With gapless disabled, the queue keeps up to 5 ms of tail samples and
applies a linear fade to and from zero at each track boundary, preserving
the number of frames. The final track ends normally.

## Verification

FLAC samples at the boundary match the unsplit sine within 1e-4.
Streaming the split FLAC pair at 48 kHz also matches the unsplit
reference within that tolerance. MP3 and AAC each present the original
sample count. A generated AAC fixture with a distinct `iTunSMPB` tag
verifies the precedence rule. The Linux ALSA null-device tests consume
exactly six seconds of frames through one CPAL stream for each pair.
Workspace tests, Clippy, and dependency checks pass locally. Hosted CI
is pending.
