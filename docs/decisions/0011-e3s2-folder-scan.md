# 0011 — E3.S2 Folder scanning and incremental imports

- **Status** Accepted (2026-11-19)
- **Story** E3.S2

## Context

A folder import needs to discover supported music files, read metadata in
parallel, and preserve each track's identity and playback history across scans.
An unavailable directory must leave existing library rows intact.

## Decision

The scanner uses WalkDir to enumerate regular files by supported extension and
Rayon to read and hash new or changed files. It resolves the selected root to
an absolute path and skips nested symlinks, including links outside that root.
Unreadable directories appear in the scan report and suppress missing-file
reconciliation for that scan. The reconciliation confirms absence or a
non-regular entry at each previously imported path before marking it missing.
[WalkDir](https://docs.rs/walkdir/2.5.0/walkdir/struct.WalkDir.html)
[Rayon](https://docs.rs/rayon/1.12.0/rayon/iter/trait.ParallelIterator.html)

Desktop V3 adds a nanosecond modification timestamp. The scanner compares it
with the stored size before opening a file. A new track gets a UUID v7. Where
a hash already exists, a changed BLAKE3 digest advances the file revision.
Timestamp-only changes refresh the fingerprint without advancing the revision.
A legacy row without a hash gets its first baseline at the existing revision.
An external tag edit that changes file bytes advances the revision. App-managed
tag writes will update the fingerprint and hash as part of E3.S4.

The importer acquires a SQLite writer transaction before reading fingerprints.
Workers stream results through a bounded queue and the calling thread applies
them to the transaction. Progress reflects the number of paths handled, with a
callback at the start, every 250 ms during that work, and when file processing
finishes. A successful return indicates the transaction committed. WAL allows
library reads during the scan and the writer transaction serializes concurrent
imports.

## Verification

The release benchmark imported 10,000 three-second stereo WAVs on a local NVMe
filesystem in 1.22 seconds and rescanned the unchanged files in 35.16 ms.
Progress events arrived approximately every 250 ms until file processing
finished. The benchmark ran immediately after fixture generation, so it may
benefit from the filesystem cache.

```bash
TMPDIR="$PWD/target" cargo bench --locked -p mt-db --bench folder_scan
```

Workspace tests and Clippy pass locally. Tests cover extension filtering,
symlink loops, missing files, reappearance, failed reads, preservation of history,
V1 migration, concurrent scans, and rollback after a database failure. Hosted
CI is pending.
