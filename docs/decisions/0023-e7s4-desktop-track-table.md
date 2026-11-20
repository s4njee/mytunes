# 0023 — E7.S4 Desktop track table

- **Status** Linux verification complete (2026-11-19)
- **Story** E7.S4

## Decision

The approved table uses fixed-height rows and a reusable pool of viewport
and overscan slots. The header and two spacers fit within the 80-row DOM
limit. The webview requests only the pages that intersect the visible
range. The grid retains keyboard focus and selection when its row slots
are reused. Extra columns are available from the header menu and are
hidden by default to preserve the approved layout.

Enter and double-click send the selected snapshot position to Rust,
which builds the queue in the current sort order. Space toggles the
existing player. Reveal resolves a stable track ID to a local path in
Rust. Deletion checks the snapshot again after the user confirms. The
Trash checkbox defaults to off. With Trash enabled, the library
commits the removal before moving files and reports any failed moves.
[Tauri Opener](https://v2.tauri.app/plugin/opener/)
[trash](https://docs.rs/trash/5.4.0/trash/)

On Linux, an inherited `appmenu-gtk-module` can cause GTK 3 to recurse
while opening the window on Wayland. MyTunes restarts itself before GTK
initializes with that module removed from its own environment. The
native File menu remains in the application window.
[GTK 3 crash report](https://bugs.debian.org/1107671)

## Verification

The 50,000-row frontend test verifies the DOM limit, modifier selection,
focus across distant scroll positions, and keyboard actions. Rust
tests verify queue order, missing-file handling, snapshot validation,
and that default removal leaves the source files in place.

In the production WebKitGTK build on Wayland, the 50,000-track fixture
averaged 60.5 fps at 200 px/s and 60.4 fps at 1,200 px/s, with 55.6 fps
at the 1% low in each run. Repeated jumps averaged 62.6 fps. The viewport
had at most 27 body rows, including spacers.

A Linux WebDriver test exercised paging, modifier selection, column
visibility, queue start by double-click and Enter, Space to pause, the
now-playing glyph, and both removal choices. The isolated session
received the expected native file-manager request and verified that
the opt-in move placed the test file in Trash. Native macOS and Windows
verification and hosted CI remain pending.
