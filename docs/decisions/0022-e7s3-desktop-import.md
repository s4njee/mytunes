# 0022 — E7.S3 Desktop library import

- **Status** Linux verification complete (2026-11-19)
- **Story** E7.S3

## Decision

The File menu and the footer add button open the native folder picker.
The main window accepts native file and folder drops. A background worker
serializes both entry points and watched-folder scans. The LCD receives
progress over a typed channel. A committed scan invalidates the open views
and reports duplicate content and individual file failures in the footer.
[Tauri Dialog](https://v2.tauri.app/plugin/dialog/)

Library preferences persist the list of watched folders and the copy option
in SQLite. A folder scan waits for two seconds of quiet before starting.
Watchers attach again at launch, and removing a watch discards its pending
scan. An unavailable directory retains its tracks and saved watch preference.
[notify](https://docs.rs/notify/8.2.2/notify/)

Copying defaults to off. When enabled, MyTunes uses the platform's Music
directory, falling back to app data if the platform supplies no Music
directory. Each source has a deterministic subdirectory under MyTunes
Media, keyed by its canonical path. This keeps filenames intact and
allows two sources with the same filename or content to coexist.
Copies are verified against the scanned digest before publication. A
failed database transaction removes the files it just created.

## Verification

The production Linux webview imported two identical WAVs through the
native File menu, displayed “Duplicate found,” and kept two tracks after
the folder was imported again. A native mixed file-and-folder drop added
one track from each input. With copying enabled, the app imported two
filesystem changes together about 2.04 seconds after the second write,
and both copies matched the original bytes. Copy and watch settings
survived relaunch. A change after watch removal did not import.

Rust tests cover overlapping selections, duplicate hashes, repeated
managed copies, missing-track scope and rollback. A frontend test checks
LCD progress and the duplicate notice. The Linux WebDriver smoke test
passed. Native macOS and Windows verification and hosted CI are pending.
