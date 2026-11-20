# 0026 — E7.S7 Library statistics

- **Status** Implementation and Linux verification complete (2026-11-20)
- **Story** E7.S7

Rust adds song count, duration, and file size while building the active
view snapshot. Cached source and sort order is shared across filtered
snapshots. Genre, artist, compilation, album, and FTS membership are
applied before adding the totals. The same snapshot provides the track
pages and selection statistics. Each selected row counts once, and
repeated tracks in a playlist count at their distinct positions.
Unknown durations and sizes contribute zero.

The footer shows the current view totals. With more than one row
selected, it shows the selected count out of the view count and the
selection's duration and size.

The optimized Linux build updated the WebKitGTK footer in 10–30 ms
across ten genre filter changes on 50,000 tracks. The check also
confirmed that changing the filter clears the previous selection
summary. Rust and frontend tests verify view and selection totals.
A prefix matching all 50,000 tracks returned the snapshot and first
200 rows in 10–17 ms across eight Linux webview calls. The Linux CI
benchmark covers the same search and page through the Rust desktop
API.

The 64-bit Windows desktop executable cross-linked after dropping the
unused shared and static library outputs from the desktop package.
The desktop program links its Rust library directly. Live macOS and
Windows verification and hosted CI remain pending.
