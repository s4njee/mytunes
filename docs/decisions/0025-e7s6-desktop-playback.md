# 0025 — E7.S6 Desktop playback

- **Status** Linux verification and macOS build complete (2026-11-20)
- **Story** E7.S6

The Rust player owns the queue independently of the window. The desktop
transport and system controls send actions to the same player. The LCD
previews a seek as the thumb moves and commits it on release. On normal
exit the app saves queue order, position, and volume in SQLite. It
rebuilds the queue from current library paths and restores it paused.

Play and skip counts use audio delivered by the engine. Seeks preserve
time already heard and add no time for the position jump. A play counts
once per pass at half the duration or four minutes, whichever comes
first. A manual skip counts after two seconds and before twenty.

Souvlaki exposes the transport, metadata, and cached artwork through
MPRIS, macOS Now Playing, and Windows SMTC. [Souvlaki](https://docs.rs/souvlaki/0.9.3/souvlaki/)

The production Linux app passed checks for desktop transport, MPRIS
metadata and artwork, seek, volume in both directions, play and skip
counts, playback while minimized, and restoration of the paused queue.
The Apple Silicon build passed using the pinned Rust toolchain. Live
macOS and Windows media controls and hosted CI remain pending.
