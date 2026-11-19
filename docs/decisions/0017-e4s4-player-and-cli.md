# 0017 — E4.S4 Shared player and CLI

- **Status** Accepted (2026-11-19)
- **Story** E4.S4

## Decision

One transport worker owns the CPAL stream, streaming queue, DSP, and
transport commands. A subscriber receives state changes, track-end
events, and position snapshots at 4 Hz while playing. Position and
track transitions derive from frames consumed by the callback. A seek
or manual track change discards queued PCM through an acknowledged
callback reset. Pause gates music while keeping the stream open.

Manual next and previous wrap in the selected shuffle order. Previous
returns to the current track's start past 4,000 ms. Song shuffle
permutes individual entries. Album shuffle permutes album groups and
preserves their supplied track order. Repeat defaults to off and can
repeat the selected track or the queue.

The CLI scans a folder into desktop SQLite and plays an album-ordered
folder queue using the shared player. Playback uses a temporary library.
The `scan` command writes to the requested database or to
`library.sqlite` in the current directory.

## Verification

Headless transport tests cover navigation, the four-second threshold,
play and pause events, frame-driven positions, automatic advancement,
shuffle, and repeat. The Linux manual check copied the FLAC fixture
pair into a temporary album folder, ran `mt-cli scan` twice, and ran
`mt-cli play <folder> --eq 3,1,-2,4,2` against a process-local ALSA
null device. The first scan imported two files and the second skipped
both. Playback emitted both tracks in order and exited successfully.
Hosted CI remains pending.
