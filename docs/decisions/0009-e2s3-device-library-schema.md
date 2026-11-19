# 0009: E2.S3 Device library schema

- **Status:** Accepted (2026-11-19)
- **Story:** E2.S3

## Context

The phone needs its own library format for synced metadata and local
player state. Sync can remove or replace metadata while earlier playback
events are still waiting for acknowledgment.

## Decision

Use a separate V1 migration and SQLite application ID (`MTDV`) for
`device.sqlite`. The device opener initializes blank files and rejects
existing files with a different identity. The desktop opener retains its
existing identity and rejects device files. Both use the same
transactional migration runner, WAL and foreign key enforcement.
[SQLite application ID](https://www.sqlite.org/pragma.html#pragma_application_id)

Device tracks store synced tags, a file revision and a path relative to
device storage. The database rejects absolute paths, empty components,
and dot segments. Artwork holds only the 300 px and 600 px thumbnails.
The album view groups compilations under “Various Artists”. A smart
playlist carries its resolved member order and the desktop retains its
rule definition.

Events store their UUID as 16 bytes, a timestamp in Unix milliseconds, a
kind, and a JSON object payload. Events survive track or playlist removal.
A pending-event index provides timestamp and UUID ordering, and a batch
acknowledgment changes only `acked` from 0 to 1. Triggers reject duplicate
inserts, edits to event data, and deletes. The duplicate guard runs before
SQLite can replace an existing row, even on a connection with recursive
triggers disabled. [SQLite conflict handling](https://www.sqlite.org/lang_conflict.html)

Player state stores one JSON value per key. The device store can use
the caller's SQLite transaction when several state changes need to
commit together.

## Consequences

The mobile facade and sync client can reuse the opener and the typed
event and state methods. The sync protocol story will define per-kind
payload fields and how the desktop processes them. Any future pruning
of acknowledged history needs an explicit change to the append-only
policy.
