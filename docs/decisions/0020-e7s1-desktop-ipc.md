# 0020 — E7.S1 Desktop IPC foundation

- **Status** Local verification complete (2026-11-19)
- **Story** E7.S1

## Decision

The Tauri window opens the desktop library in the app-data directory.
`mt-app` builds an ordered ID snapshot for a library or playlist view.
The Rust query combines literal prefix search with optional browser
filters. A view returns its ID, row count, and revision. Row requests
return at most 200 tracks in view order. The backend retains eight
recent views, and the frontend adapter caches eight pages per view.
Library revision notifications invalidate cached pages.

Tauri Specta exports typed commands, a discriminated `AppError`, and
the `LibraryChanged` and `DeviceOnline` events. The player, scan, and
sync streams each keep one channel for the main window. Subscription
replays the latest available update and replaces its previous channel.
SQLite paging and artwork reads run off the webview and async runtime
threads.

The `mtart` handler serves JPEGs by content digest at the supported
300 and 600 px sizes. Successful responses use immutable caching.
The frontend uses `artUrl` to obtain the platform-specific URL.
The main window capability lists its six commands and the two event
listener permissions. Its CSP limits subresources to the configured
origins, and the window rejects top-level navigation away from the app
and, in development, the configured Vite origin.

## Verification

Rust tests cover view order, repeated playlist tracks, search and
filter combination, page limits, cache expiry, channel resubscription,
and artwork routing. Vitest exercises the frontend page cache and both
custom-scheme URL forms. The Linux Tauri WebKitGTK smoke test opened
and paged a view, received the initial transport snapshot, checked a
typed oversized-page error, and blocked a remote image and top-level
navigation with zero requests to the test server. The 50k-row Linux
query benchmark measured a slowest snapshot case of 20.33 ms and a
200-row page at 0.41 ms. Hosted CI is pending.
