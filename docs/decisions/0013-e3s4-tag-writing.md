# 0013 — E3.S4 Tag writing

- **Status** Accepted (2026-11-19)
- **Story** E3.S4

## Context

Get Info needs to write edits into the local audio file while keeping the
track's library identity and sync file revision. An encoding failure or a
read-only file must leave the library row unchanged.

## Decision

Load existing tags with Lofty and change only requested fields. Store track
and disc numbers and totals independently. Update the MyTunes POPM entry
for ID3, preserving other applications' ratings and play counters. Use a
single native rating in MP4 and Vorbis comments. Front-cover edits preserve
other typed pictures. MP4 presents untyped cover slots, so replacement or
removal applies to those slots.

Stage changes in a temporary file beside the original. Read back the metadata,
prepare artwork, update the fingerprint, and sync the replacement before
publishing it with an atomic rename. The editor compares the source digest
with the imported digest and requests a rescan if it changed. It shares the
scanner's library mapping and artwork grouping, with file revision advancement
disabled. [Atomic file replacement](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist)

The library writer transaction serializes edits and imports. Retain the
original via an adjacent hard link or a copy until SQLite commits. If the
commit returns an error, restore the original file and its timestamp. A
process crash between replacement and database commit can leave the file
ahead of SQLite and an adjacent backup on disk. A rescan then reconciles
the library from the file. Filesystem rename and SQLite commit are
separate operations.

## Verification

Generated fixtures round-trip all requested fields across MP3, AAC,
ALAC, FLAC, WAV, AIFF, Vorbis, and Opus. Library tests verify unrelated
metadata, playback history, the unchanged rescan path, read-only rejection,
and restoration after a deferred SQLite constraint fails at commit.
Hosted CI remains pending.
