# Measurements

Raw output from plan E1 technical spikes. Each file notes the story that
produced it; the analyzed numbers live in the matching ADR in
`docs/decisions/`.

## `e1-s1-webkitgtk-bench.jsonl`

E1.S1 auto-benchmark (one JSON line per run, written by the spike's
`benchmark_report` command). Produced by:

```
pnpm tauri build --debug --no-bundle   # production frontend, debug Rust
MT_SPIKE_AUTORUN=full \
MT_SPIKE_RESULTS=<path>/e1-s1-webkitgtk-bench.jsonl \
  ../../target/debug/mytunes-desktop
```

on Linux/Wayland, WebKitGTK, AMD Radeon, 60 Hz display, 2026-11-18. The
second line (`position-stream-done: 2400 events over 600s`) is the AC4
ten-minute Channel stream marker.

## `e1-s1-webkitgtk-rss.tsv`

`timestamp_s pid comm VmRSS_kb` of the app's WebKitWebProcess, sampled
every 15 s by the outer harness during the same run. Some sample rows
belong to helper shells matched by `pgrep -f` — filter on the
`WebKitWebProces` comm column.
