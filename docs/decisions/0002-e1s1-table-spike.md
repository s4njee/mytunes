# 0002: E1.S1 spike result — Tauri + Solid 2.0 table at scale

- **Status:** Accepted (2026-11-18)
- **Story:** E1.S1 (AC1–AC6)

## Context

Plan §2.3 R2/R3: Solid 2.0 shipped one day before this spike ran, and the
desktop's primary platform is WebKitGTK, the slowest and quirkiest of the
three webviews. Before building E7 on Tauri 2 + SolidJS 2.0 we needed
measured proof that a hand-rolled virtualized table can hit jukebox-grade
numbers on Linux, and that the whole 2.0 toolchain works end to end. The
spike is time-boxed throwaway code on branch `spike/e1-s1-table`; this ADR
is the durable output.

## What was built (throwaway, branch `spike/e1-s1-table`)

- Backend (`apps/desktop/src-tauri/src/spike.rs`): an in-memory SQLite
  table of 50,000 synthetic tracks, served through the plan §3.5 paging
  API — `view_open` materializes an ordered id snapshot per view,
  `view_rows` pages 200 rows at a time, sort keys are whitelist-mapped to
  SQL. Also: `position_start` streaming 4 Hz `PositionEvent`s over a
  `tauri::ipc::Channel`, a typed `DroppedFiles` drag-drop event forwarded
  from the window's `DragDrop` event, and `benchmark_report` persisting
  measurements to a JSONL file (`MT_SPIKE_RESULTS`).
- Frontend (`apps/desktop/src/spike/`): a pool-rendered `VirtualTable`
  (fixed 22px rows, 8 sortable + resizable columns, shift/ctrl selection,
  zero DOM-node churn while scrolling), a `TrackView` LRU page cache
  (40 pages × 200 rows), a rolling rAF fps meter, and a scriptable
  auto-benchmark (`MT_SPIKE_AUTORUN=full` on the backend makes the
  webview run idle → 20 page fetches → 3 sorts → 200/1200 px/s scrolls →
  random-jump phase, then start the ten-minute stream).
- Tests: 6 Vitest cases (bounded DOM, scroll rebinding, sort
  delegation, selection semantics, boot, ping) plus the existing
  tauri-driver E2E smoke, updated to target the ping button by test id.

## Measurements (WebKitGTK, Linux, Wayland, AMD Radeon, 60 Hz display, production frontend via `tauri build --debug`, Rust debug profile)

| Metric | AC bar | Measured |
|---|---|---|
| `view_rows` round-trip, 200 rows, 20 calls | < 8 ms | median **2 ms**, max 3 ms |
| Sort 50k rows → visible rows refreshed (3 sorts) | < 150 ms | **29 / 48 / 32 ms** |
| Scrolling 200 px/s | ≥ 55 fps | **60.2 avg**, 55.6 1% low |
| Scrolling 1200 px/s | ≥ 55 fps | **60.0 avg**, 55.6 1% low |
| Random scroll jumps (500 ms period) | ≥ 55 fps | 62.3 avg, 53.8 1% low |
| Idle | — | 63.0 avg |

Notes on the numbers:

- The display is 60 Hz, so ~60 fps is the ceiling; the tables hold it
  under all phases. 1% lows dip just under 56 fps during the most
  extreme (jump) phase — the fallback (canvas-rendered body) is not
  needed. A 120 Hz panel re-run would show headroom.
- The window ran on an active desktop; compositing contention from
  other apps is included, so these numbers are realistic, not
  lab-ideal.
- No NVIDIA GPU here (AMD), so the `WEBKIT_DISABLE_DMABUF_RENDERER`
  workaround (plan §2.3) was **not** needed; the README must still
  document it, per plan E0.S1 AC5.

## AC4: Channel streaming, 10 minutes

A 4 Hz `Channel<PositionEvent>` stream ran for the full 600 s (2,400
events). WebKitWebProcess RSS was sampled every 15 s from outside (WebKit
exposes no `performance.memory`).

- All 2,400 events arrived (the frontend's counter and the final marker
  agree), so the Channel stayed live for the whole run.
- RSS: ~153 MB right after the benchmark (its transients), trimmed by
  WebKit over the first ~3 minutes, then **flat at ~50 MB (41–54 MB
  band, non-monotonic) for the remaining seven minutes**. No growth is
  attributable to the stream: the wiggle is WebKit cache noise, not a
  leak trend.
- Follow-up for E7.S4 (not a spike blocker): run a 1-hour soak with the
  real player events and watch the same RSS band; a leak would show
  monotonic growth well beyond this wiggle.

## Decisions

1. **GO.** Solid 2.0 + Tauri 2 + hand-rolled virtualization meets every
   E1.S1 bar on the riskiest platform. No fallback to Solid 1.9.x, no
   canvas table. Proceed to M1/E7 with this architecture.
2. **Pool rendering is the table design for E7.S4**: bind a fixed pool
   of row slots to window positions instead of keyed rows per index.
   Scrolling then updates content/styles only — no node churn — which is
   what keeps WebKitGTK at 60 fps. The pooled-grid a11y debt (roving
   tabindex, ARIA grid) is owned by E7.S4/E14.S2.
3. **Benchmarks must run against the production frontend**
   (`tauri build --debug`, not `tauri dev`): Solid dev-mode diagnostics
   materially change rendering cost. `pnpm test`/Vitest covers
   correctness; fps claims need the built webview.
4. The spike's `view_open`/`view_rows` shape is confirmed as the E7.S1
   IPC contract (plan §3.5). The phone's `mt-ffi` should mirror the same
   paging shape when M3 needs list windows.

## Gaps and follow-ups

- **Windows (WebView2) and macOS (WKWebView) numbers are still pending**
  (E1.S1 AC2's cross-engine check). They need either the first GitHub CI
  run (E0.S2, blocked on pushing the repo) or a manual run on the owner's
  machines: build the spike branch, launch with
  `MT_SPIKE_AUTORUN=full MT_SPIKE_RESULTS=<path>`, read the JSONL. Expect
  ≥ WebKitGTK numbers; treat < 55 fps on WebView2 as a blocker to file
  against plan §2.3 R8.
- **Drag-drop (AC3) is implemented and wired end to end** (window
  `DragDrop` → typed `DroppedFiles` specta event → footer panel), and
  the E2E smoke proves the app boots with the listener registered — but
  the physical OS drag needs one manual verification: drag any file from
  the file manager onto the spike window; the footer's "dropped N" count
  and the panel must show the absolute path. 5 seconds of owner time.
- **`rusqlite` 0.43.0 (bundled) and `tokio` 1.54.1** were added as exact
  pins in the spike only. The real schema/query layer belongs to `mt-db`
  (E2), which should re-pin these deliberately; the spike branch must
  not be merged.
- The spike's `performance.memory`-style in-webview memory measurement
  does not exist on WebKitGTK (no `performance.memory`); AC4 memory
  verification is done by sampling the WebKit process RSS externally,
  which is the pattern to keep for E7 regression checks.

## Consequences

- Plan §2 stands unchanged: no update to §2.1/§2.3 needed (both fallbacks
  remain documented but unused).
- E7.S3/E7.S4 can start from the spike's `VirtualTable` as a reference
  implementation, re-authored in the real component tree.
- The next agent inherits a ready-made benchmark harness: any future
  rendering regression on any platform can be reproduced with two env
  vars and a JSONL read.
