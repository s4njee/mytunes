# 0028 — E5.S2 Mobile Flutter–Rust bridge

`openDeviceDb` returns a handle to one migrated device database. Rust
pages albums and songs, keeps disc order within albums and preserves
repeated playlist entries. Playback resolves track IDs against the device
database and uses files beside it. Artwork thumbnails are published to a
local cache as complete files. Settings and events use the device
store's existing JSON representation and append-only event log.

Flutter Rust Bridge dispatches library operations and player commands on
Rust worker threads. The shared audio engine starts when playback controls
or events are first requested. Cancelling the Dart stream releases its
native subscription. Android gives CPAL the application context before
Flutter starts, builds native dependencies with Gradle's NDK, and uses
the plan's Android 8 minimum for AAudio.

Rust tests cover populated libraries, queue ordering, and persistence.
The Android emulator integration tests call the local bridge from Dart,
including stream delivery, cancellation, and empty-queue error messages.
A temporary panic probe confirmed that the bridge reports a Dart
`PanicException` with the Rust message and can serve later requests. The
GitHub Actions Android job runs the Dart integration tests after uploading
the normal APK. Native iOS verification and hosted CI remain open.

Network sync joins this facade in M4 when `mt-sync-client` is implemented.
