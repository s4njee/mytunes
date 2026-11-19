# 0018 — E4.S5 Synthesized wheel click

- **Status** Implemented, pending device verification (2026-11-19)
- **Story** E4.S5

## Decision

`Player::play_sfx(Click)` requests a 12 ms synthesized click. On desktop
and Android, the output callback reads the request at the start of the
next buffer and mixes the precomputed waveform into PCM. Multiple
requests within one callback period coalesce. The stream stays open
while paused and the callback leaves queued music in the ring.

iOS uses AudioToolbox System Sound Services for the same waveform. This
is a deviation from the shared callback path in AC1. The mobile app
needs the `playback` category for lock-screen music playback. System
sounds use their own low-priority audio category and the default UI
sound behavior. System Sound Services schedules the click and plays it
on the device speaker, with system volume and no app-controlled route.
Apple's archived guide also describes possible vibration in Silent mode.
[Audio session categories](https://developer.apple.com/library/archive/documentation/Audio/Conceptual/AudioSessionProgrammingGuide/AudioSessionCategoriesandModes/AudioSessionCategoriesandModes.html)
[System sounds and audio sessions](https://developer.apple.com/library/archive/documentation/MusicAudio/Conceptual/CoreAudioOverview/CoreAudioEssentials/CoreAudioEssentials.html)
[System sound playback](https://developer.apple.com/documentation/audiotoolbox/audioservicesplaysystemsound%28_%3A%29)
[UI sound property](https://developer.apple.com/documentation/audiotoolbox/kaudioservicespropertyisuisound)

## Verification

The Linux callback test exercises 256 frames at 48 kHz with the debug
allocation guard enabled. The first nonzero frame is one frame into the
next buffer. Including the time to that buffer, the calculated bound is
5.36 ms. The tests verify a click shorter than 15 ms, stereo output,
headroom over music, and preservation of the paused music queue. The
player smoke test also requests a click through the public API while
paused on a Linux ALSA null device.

The full audio crate checks for `aarch64-apple-ios` with Rust 1.98.0
and Xcode 27.2. A SwiftUI probe linked the current `Player` into an
arm64 iPhone app. On the iOS simulator the same probe opened a
`playback` session, loaded a synthetic FLAC track, and called
`Player::play_sfx(Click)` successfully with music paused and playing.
The physical build awaits code signing.

On the physical iPhone, exercise the click with Silent mode off and on,
both while music is paused and while it is playing. Record whether the
click is audible, whether it vibrates, and whether changing Silent mode
interrupts the music. Measure click onset with an audio capture that
records both the trigger and the device speaker.
