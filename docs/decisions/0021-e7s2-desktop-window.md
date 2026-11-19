# 0021 — E7.S2 Desktop main window

- **Status** Local verification complete (2026-11-19)
- **Story** E7.S2

## Decision

The main Tauri window renders the approved `LibraryWindow` components
with the local SQLite library. It reads the first page of the active
view through the E7.S1 adapter. E7.S4 will add viewport-driven paging.
The library state refreshes on a revision event.

The window follows system appearance until the user chooses Light or
Dark. SQLite stores the choice, sidebar width, column browser height
and visibility, artwork visibility, column widths and visibility, and
the selected sort. Column headers request a new sorted view from Rust.

Tauri restores the native position, size and maximized state before
showing the main window. The single-instance plugin focuses the first
window after a second launch. Rust manages both plugins.
[Window State](https://v2.tauri.app/plugin/window-state/)
[Single Instance](https://v2.tauri.app/plugin/single-instance/)

The pinned Tauri 2 single-instance plugin still uses zbus 5 on Linux.
Three advisory exceptions record the upstream move of its standalone
dependencies into zbus 6. The exceptions can be removed when the plugin
updates its dependency. [zvariant](https://rustsec.org/advisories/RUSTSEC-2026-0370.html)
[zbus_names](https://rustsec.org/advisories/RUSTSEC-2026-0371.html)
[zvariant_derive](https://rustsec.org/advisories/RUSTSEC-2026-0372.html)

## Verification

The packaged Linux app restored a saved dark theme, sidebar width,
column visibility and width, and album sort in a WebKitGTK reload.
A native X11 test reopened the window at the same position and
1092 × 710 size, and a second launch focused the original window and
exited. On macOS, both themes rendered in WKWebView using the built
frontend with fixture IPC responses. Hosted CI remains pending.
