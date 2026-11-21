# Handoff: iPod-style MP3 player (iOS)

## Overview
A local-library music player for iOS, explored in three navigation directions on one canvas:

- **1a — Wheel-first.** A click-wheel reinterpretation. A physical ring below the display is the only navigation: drag around it to move a selection or scrub, press the centre to confirm. Five screens reachable: Menu, Albums, Now Playing, Queue, Equalizer.
- **1b — Modern, no wheel.** Standard iOS navigation (tab bar, tappable rows, draggable seek bar) with the same visual identity. Same five screens.
- **1c — Hybrid.** Modern Now Playing layout, but the wheel survives as a scrub arc wrapped around the artwork; everything else is ordinary taps.
- **2a — Light theme.** 1a's wheel-first design in the original iPod palette: white polycarbonate body, brushed-silver wheel, chrome centre button, and a solid iPod-blue selection bar. Same five screens, same interaction model; **only the palette differs.**

Build the two palettes as one themed component with a swappable colour set, not two screens.

Library source is **local files the user owns** — no streaming, no accounts, no network states.

## About the Design Files
The files in this bundle are **design references created in HTML** — prototypes demonstrating intended look and behavior. They are not production code to copy. `iPod Player.dc.html` uses a bespoke streaming-template runtime (`support.js`) that exists only in the design tool; do not port that runtime.

The task is to **recreate these designs in the target codebase's existing environment** — for iOS this most likely means SwiftUI, using the app's established patterns, components, and colour/type assets. If no codebase exists yet, pick the framework appropriate to the platform (SwiftUI for a native iOS app) and implement there.

Open `iPod Player.dc.html` in a browser to interact with all three directions side by side.

## Fidelity
**High fidelity.** Colours, type, spacing, and interactions are final-intent and specified exactly below. Recreate the UI to match. The one deliberate exception: **album artwork is placeholder** — flat colour tiles carrying a small mono caption. Real artwork drops into the same frames (same size, radius, shadow); the caption overlay disappears when real art is present.

Device frame, status bar, and home indicator in the prototype come from a generic iOS mock frame. **Do not implement them** — they represent the OS.

---

## Design Tokens

### Colour — dark theme (default)
| Token | Value | Use |
|---|---|---|
| `ink` | `#0E1116` | Screen background; centre-button face; text on amber |
| `panel` | `#151A21` | Wheel deck (1a bottom panel) |
| `chrome` | `rgba(23,28,35,.92)` | Tab bar fill (1b), over a 20px backdrop blur |
| `amber` | `#F0A23C` | Single accent: selection, progress, active tab, active EQ band, primary button |
| `amber-14` | `rgba(240,162,60,.14)` | Selected row background |
| `amber-42` | `rgba(240,162,60,.42)` | Inactive EQ band fill |
| `amber-35` | `rgba(240,162,60,.35)` | Centre-button hairline ring |
| `text` | `#EDF0F3` | Primary text |
| `text-90` | `rgba(237,240,243,.9)` | Track titles in lists |
| `text-70` | `rgba(237,240,243,.7)` | Unselected menu rows, wheel glyphs |
| `text-55` | `rgba(237,240,243,.55)` | Secondary text (artist) |
| `text-42/40` | `rgba(237,240,243,.42)` / `.4` | Tertiary text (metadata, counts) |
| `text-35/32/28` | `rgba(237,240,243,.35)` / `.32` / `.28` | Quaternary — inactive icons, chevrons, drag handles |
| `hairline` | `rgba(255,255,255,.06)` | Row separators (1b) |
| `rule` | `rgba(255,255,255,.09)` | Section rules (1a header, EQ divider) |
| `track-inactive` | `rgba(255,255,255,.12)`–`.16` | Progress/slider/toggle track, inactive vol bars |
| `well` | `rgba(255,255,255,.05)`–`.06` | EQ band well |

The wheel body is the only gradient in the system, standing in for anodized aluminium:
```
background: linear-gradient(160deg, #2B333D, #1A2027 62%, #232B34);
box-shadow: inset 0 1px 0 rgba(255,255,255,.11),
            inset 0 -1px 0 rgba(0,0,0,.5),
            0 10px 26px rgba(0,0,0,.45);
```

**Placeholder artwork palette** — six fixed `[fill, caption-colour]` pairs, indexed by album:
```
0 #F0A23C / #14181D      3 #C3CBD3 / #181C21
1 #2E3A46 / #DDE4EA      4 #3A4C5A / #DDE4EA
2 #8C5A3C / #F5E9E1      5 #5E6B57 / #EDF2E8
```

### Colour — light theme (original iPod)
Same token roles, different values. Everything else — type, spacing, radii, geometry, interaction — is unchanged.

| Token | Value | Use |
|---|---|---|
| `bg` | `#FBFBF9` | Screen (the LCD) |
| `deck` | `#EFEFEB` | Wheel deck — white polycarbonate body |
| `accent` | `#2F6DB5` | iPod blue: selection bar, progress, active EQ band, format badge, eyebrow |
| `accent-half` | `rgba(47,109,181,.4)` | Inactive EQ band fill |
| `text` | `#14171A` | Primary text |
| `text-90 / 80 / 78` | `rgba(20,23,26,.9)` / `.78` | Track titles, setting labels |
| `text-66 / 56 / 50` | `rgba(20,23,26,.66)` / `.56` / `.5` | Menu rows, artist, timecode, wheel glyphs |
| `text-46 / 42 / 38 / 34` | `rgba(20,23,26,.46)` / `.42` / `.38` / `.34` | Metadata, counts, gain values, idle hint |
| `dot` | `rgba(20,23,26,.17)` | Unselected menu dot; EQ zero line |
| `well` | `rgba(20,23,26,.06)` | EQ band well |
| `track-inactive` | `rgba(20,23,26,.13)`–`.14` | Progress track, inactive volume bars |
| `rule` | `rgba(20,23,26,.09)`–`.1` | Header rule, EQ divider, deck top border |

**Selection reads differently in light.** Dark theme tints the row (`amber-14` background, amber dot, white label). Light theme fills it — the whole row goes solid `#2F6DB5` with white text, a `rgba(255,255,255,.85)` dot, and secondary text at `rgba(255,255,255,.7–.75)`, matching how the click-wheel iPods highlighted a row. Row radius drops from 10 to **5** so the bar reads as a block. In the Queue, a selected row's title is white; the *currently playing* row (when not selected) is blue.

Silver wheel:
```
background: linear-gradient(160deg, #FDFDFC, #E1E1DD 62%, #F1F1ED);
box-shadow: inset 0 1px 0 #fff,
            inset 0 -1px 0 rgba(20,23,26,.14),
            0 8px 18px rgba(20,30,45,.16);
```
Chrome centre button: `#FBFBFA` face, `inset 0 0 0 1px rgba(47,109,181,.4)`, `0 2px 9px rgba(20,30,45,.2)`.

Light shadows: album tile `0 6px 16px rgba(20,30,45,.14)`, selected `0 0 0 2px #2F6DB5, 0 6px 16px rgba(20,30,45,.18)`, artwork `0 10px 24px rgba(20,30,45,.18)`.

Light placeholder artwork palette — cooler and paler so tiles sit on white:
```
0 #2F6DB5 / #EAF1FA      3 #8E9AA5 / #F2F5F8
1 #C9CFD6 / #22282E      4 #DCD8CF / #3A3730
2 #B98A5E / #FFF6EE      5 #7E8C74 / #F2F6EE
```

The status bar switches to dark glyphs in light theme (`UIStatusBarStyle.darkContent`).

### Typography
Two families only.

- **IBM Plex Sans** — all UI text. Weights 400 / 500 / 600 / 700.
- **JetBrains Mono** — everything numeric or machine-ish: timecodes, durations, counts, track numbers, format badges, EQ gains, Hz labels, wheel labels, tab labels. Weights 400 / 500.

| Role | Spec |
|---|---|
| Large title (1b header) | Plex Sans 700 / 30px / 1 / `-0.01em` |
| Now Playing title (1b) | Plex Sans 600 / 27px / 1.15, `text-wrap: pretty` |
| Now Playing title (1c) | Plex Sans 600 / 26px / 1.2, centred |
| Now Playing title (1a) | Plex Sans 600 / 24px / 1.15 |
| Menu row (1a) | Plex Sans 500 / 19px / 1.2 |
| Library row (1b) | Plex Sans 500 / 18px / 1.2 |
| Artist (1b Now Playing) | Plex Sans 400 / 17px / 1.3 |
| Setting row label (1b) | Plex Sans 400 / 17px / 1 |
| Queue title (1b) | Plex Sans 500 / 16px / 1.2 |
| Artist (1a / 1c) | Plex Sans 400 / 15–16px / 1.3 |
| Queue title (1a) | Plex Sans 500 / 15px / 1.2 |
| Album title (grid) | Plex Sans 500 / 13px (1a) · 14px (1b) / 1.25 |
| Album artist (grid) | Plex Sans 400 / 11–12px / 1 |
| Timecode | Mono 400 / 12px / 1 |
| Metadata, counts, durations | Mono 400 / 11–12px / 1 |
| Section eyebrow (1a header) | Mono 500 / 11px, `letter-spacing .16em`, uppercase |
| Eyebrow (1c "NOW PLAYING") | Mono 500 / 10px, `letter-spacing .2em` |
| Wheel MENU label | Mono 500 / 10px, `letter-spacing .2em` |
| Wheel centre label | Mono 500 / 9px / 1.4, `letter-spacing .16em`, centred |
| Wheel transport glyphs | Mono 400 / 14–15px |
| Tab bar label | Mono 500 / 9.5px, `letter-spacing .06em` |
| Art caption overlay | Mono 500 / 10–11px / 1.3, `letter-spacing .06–.08em`, uppercase |
| EQ Hz / gain | Mono 500 / 10–11px · Mono 400 / 10px |

### Spacing
4px base. Screen gutter **22px** (1a), **24px** (1b), **28px** (1c). Section gaps 12 / 14 / 18 / 22 / 26 / 38px. Row heights: 42, 48, 54, 56, 60, 62px. Grid gap 14px (1a) / 16px (1b).

### Radius
2, 3 (progress) · 6 (1a album tile) · 7, 8, 9, 10 (rows, list tiles) · 12 (1b album tile) · 14 (1c artwork, 1b EQ well) · 16 (toggle) · 18 (1b artwork) · 22 (tab bar) · 50% (centre button, play button, dots)

### Shadow
| Use | Value |
|---|---|
| Wheel | see gradient block above |
| Centre button | `inset 0 0 0 1px rgba(240,162,60,.35), 0 4px 14px rgba(0,0,0,.6)` |
| Album tile (grid) | `0 8px 20px rgba(0,0,0,.35)`; selected `0 0 0 2px #F0A23C, 0 8px 20px rgba(0,0,0,.4)` |
| Artwork 1a | `0 14px 30px rgba(0,0,0,.5)` |
| Artwork 1b | `0 26px 60px rgba(0,0,0,.55)` |
| Artwork 1c | `0 22px 50px rgba(0,0,0,.55)` |
| Tab bar | `0 12px 30px rgba(0,0,0,.5)` + `1px solid rgba(255,255,255,.08)` |

---

## Screens / Views

All screens are 402 × 874pt (iPhone logical size in the mock). Content begins 62pt from the top (below the status bar); 1c begins at 70pt.

### 1a — Wheel-first

Vertical split: **display area** `flex:1`, padding `62 22 14`; **wheel deck** fixed at the bottom, padding `20 0 34`, background `panel`, top border `rule`, contents centred with 14px gap.

**Display header** (all 1a screens): a row with a 12pt bottom padding and `rule` bottom border. Left — context eyebrow in amber (`Library` / `Albums` / *album title* / `Queue` / `Equalizer`). Right — metadata in `text-40` (`7 songs · 6 albums` / `6 albums` / *album tag* / `7 tracks` / `Custom`).

**Menu** — five 54pt rows, 12pt horizontal padding, radius 10, 12pt gap between the parts: a 6pt dot, the label, then a right-aligned count in mono. Selected row: background `amber-14`, label `#fff`, dot amber. Unselected: transparent, label `text-70`, dot `rgba(255,255,255,.16)`. Rows and their destinations: Now Playing → Now Playing · Albums (6) → Albums · Songs (7) → Queue · Queue (7) → Queue · Equalizer → Equalizer.

**Albums** — 2-column grid, 14pt gap, 14pt top padding, `align-content: start`. Each cell: square tile (radius 6, placeholder fill, caption bottom-left with 10pt padding), then title (amber when selected, else `text`) and artist. Selected tile carries the 2pt amber ring.

**Now Playing** — vertically centred, 22pt gaps.
1. Row: 140 × 140 artwork (radius 8) + text block bottom-aligned — title 24px, artist 15px, then `FLAC · 24/96` in amber mono with `.1em` tracking.
2. Progress: 3pt track (radius 2) with amber fill; below it elapsed / `-remaining` in mono `text-50`, 9pt above.
3. Volume: `VOL` label (mono 10px, `.16em`) + 18 bars, 3pt gap, heights alternating 5/7/9px, filled amber up to the volume fraction, rest `rgba(255,255,255,.14)`.

**Queue** — seven 48pt rows, radius 9. 20pt zero-padded track number in mono, then title + artist stacked, then duration. Selected row: `amber-14` background. **Currently playing** row: number and title amber (independent of selection).

**Equalizer** — a 190pt-tall band row (5 bands, 12pt gap) over a divider and three status rows.
- Each band: a full-width well (`rgba(255,255,255,.05)`, radius 3) with a centre zero line at 50% (`rgba(255,255,255,.14)`); the gain bar grows from centre — `height: |gain|/6 × 50%`, anchored `bottom: 50%` for positive gain and `bottom: 50% − height` for negative. Active band amber, others `amber-42`.
- Below each: Hz label (amber if active, else `text-50`) and signed gain (`+3`, `-2`).
- Status rows: 42pt each, label `text-80` left, value right in mono — `ON` amber / `OFF` `text-35`. Rows: Gapless playback · Volume limit · Shuffle album order.

**The wheel** — 252pt circle, `touch-action: none`, `user-select: none`, `cursor: grab`.
- `MENU` label at `top:16`, centred. Its hit area: `top:0; left:76; right:76; height:56`.
- `◀◀` left and `▶▶` right: 56 × 56 hit areas at `top:98`.
- `▶ ❙❙` at the bottom: `left:76; right:76; height:56`. This label is **static** — it names the button, it does not reflect state.
- Centre button: 104pt circle, `ink` face, amber hairline, label in amber that reflects context — `SELECT` (Menu) · `PLAY` (Albums, Queue) · `PAUSE`/`PLAY` (Now Playing) · `BAND <hz>` (Equalizer).
- Hint line under the wheel, mono 10px `.14em`, `text-30` normally and **amber while dragging**: `DRAG RING TO BROWSE` / `DRAG RING TO SCRUB` / `DRAG RING TO SET GAIN`.

There are no tick marks on the wheel — they were removed deliberately. The ring reads as plain machined metal.

> The five text tabs rendered *below* the 1a device are a review shortcut for jumping between screens. **Not part of the app** — do not implement.

### 1b — Modern, no wheel

Header: padding `62 24 10`, large title left (`Library` / `Albums` / `Playing` / `Queue` / `Sound`), metadata right in mono `text-40` (`412 MB on device` / `6 albums` / *album title* / `Up next` / `Custom preset`). Scrolling content below, 24pt gutter, with a 120pt bottom spacer clearing the tab bar.

**Library** — six 60pt rows separated by `hairline`: 34pt rounded-square colour swatch (radius 8), label 18px, count in mono, `›` chevron in `text-28`. Rows: Albums 6 · Artists 6 · Songs 7 · Playlists 3 · Recently added 2 · Imported folders 4.

**Albums** — 2-column grid, 16pt gap, square tiles at radius 12 with the caption bottom-left, title and artist below. Tap plays the album's first track and navigates to Playing.

**Now Playing** — 26pt gaps: full-width square artwork (radius 18, 20pt caption padding) · title 27px + artist 17px · seek bar (5pt track, radius 3, amber fill) inside a 24pt tap row, elapsed / `-remaining` below · transport row `space-between` — `◀◀` and `▶▶` at 20px in `text-70`, centre a **78pt amber circle** with an `ink` glyph that *does* reflect state (`❙❙` playing / `▶` paused).

**Queue** — seven 62pt rows separated by `hairline`: 36pt art thumbnail (radius 7), title + artist, duration, then a three-bar drag handle (15 × 1.5pt bars, 3pt gap, `text-30`). Current track's title is amber.

**Sound (EQ)** — 220pt band row, 14pt gap, same centre-anchored bar geometry as 1a but with a radius-14 well and radius-10 bar; Hz label 11px below each, amber when active. Tapping a band selects it. Below, three 56pt setting rows separated by `hairline` with iOS-style switches: 50 × 30 track (radius 16, 3pt padding), 24pt white knob, track amber when on and `rgba(255,255,255,.16)` when off. Tapping toggles.

**Tab bar** — floating: `left:14; right:14; bottom:26`, radius 22, `chrome` fill + 20px backdrop blur, 1pt `rgba(255,255,255,.08)` border, 8pt padding. Five equal items, each an 18pt icon placeholder over a mono label, 6pt gap, 8pt vertical padding. Active icon and label amber; inactive `text-32` / `text-45`. Items: LIBRARY (radius 3) · ALBUMS (3) · PLAY (50%) · QUEUE (2) · EQ (5). **The square/circle icon shapes are placeholders** — substitute real symbols.

### 2a — Light theme (wheel-first)
Structurally identical to 1a — same layout, sizes, screens, wheel geometry, hit areas, labels, and behavior. Substitute the light colour set above. The only structural change is the selection treatment described under the light palette (solid bar, radius 5). The prototype holds its navigation state separately from 1a purely so both frames can be driven independently on one page; a real app has one deck and a theme setting.

### 1c — Hybrid

Single Now Playing screen, padding `70 28 40`.
- Top row: `NOW PLAYING` eyebrow left in `text-42`, format badge right in amber.
- Centre, 38pt gaps: the **scrub arc** — a 300pt square containing 60 tick marks on a circle of radius 138pt (2pt wide, 14pt long every fifth, else 9pt; ticks up to the current position amber, the rest `rgba(255,255,255,.14)`), with the artwork inset 34pt inside it (radius 14, 18pt caption padding). Below: title 26px, artist 16px, then `elapsed / total` in mono `text-42` with `.08em` tracking, all centred.
- Bottom transport: `◀◀` / `▶▶` at 18px in `text-60` flanking a **72pt outlined** play button — transparent fill, `inset 0 0 0 1.5px` amber, amber glyph reflecting state.

---

## Interactions & Behavior

### The wheel (1a) — the load-bearing interaction
Pointer-based, works with touch, mouse, and pencil.

1. **Down**: capture the pointer; record the angle from the ring's centre to the pointer, `atan2(dy, dx)` in degrees; reset the accumulator.
2. **Move**: compute the new angle, take the delta, **normalise across the ±180° seam** (add or subtract 360 when `|delta| > 180`) — without this the wheel jumps a full turn each time the finger crosses the top. Add the delta to the accumulator.
3. **Detent**: every **12°** of accumulated rotation emits one step and subtracts 12° from the accumulator, so a fast drag emits several steps in one move event. Each step fires a **haptic pulse** — in the web prototype `navigator.vibrate(6)`; on iOS use `UIImpactFeedbackGenerator(style: .light)` or `.selection` for list navigation, prepared ahead of the gesture so it fires without latency.
4. **Up / cancel**: clear the drag state.

There is **no inertia or momentum** — the mapping is 1:1 with detents and stops when the finger stops. If you add momentum later, keep the detent haptic firing per emitted step.

A step means different things per screen: Menu / Albums / Queue → move the selection by one, clamped at both ends (no wrap). Now Playing → seek ±4 seconds, clamped to the track. Equalizer → adjust the selected band by ±1 dB, clamped to ±6.

Button behavior: **MENU** returns to the Menu screen and resets the selection to 0 (from Menu it is a no-op). **Centre** confirms per context — open the selected menu screen · play the selected album (its first track) and go to Now Playing · play the selected queue track and go to Now Playing · toggle play/pause · advance to the next EQ band, wrapping. **◀◀** restarts the track if more than 4 seconds have elapsed, otherwise goes to the previous track (wrapping) — standard player behavior. **▶▶** goes to the next track, wrapping. Both reset position to 0. **Every button press pulses haptics.**

### The scrub arc (1c)
Absolute, not relative: pointer angle maps directly to position. Rotate the measured angle by +90° so 12 o'clock is zero, normalise into 0–360°, then `position = angle / 360 × duration`. Scrubbing begins on pointer-down (the down event runs the same handler as move, so a tap jumps).

### 1b
Taps only. Library rows navigate; album tiles play and navigate to Playing; queue rows play and navigate; the seek bar maps tap x-position to time (`(x − left) / width × duration`); EQ bands select on tap; switches toggle. The drag handles on queue rows are **visual only** in the prototype — implement real reordering.

### Playback
A 1-second tick advances position while playing; on reaching the duration, position resets to 0. In the prototype it loops the same track — **in the real app it should advance to the next track in the queue.** Position is clamped to `[0, duration)`.

### Not covered by these mocks
Empty library, import/file-picking flow, loading and buffering, missing-artwork fallback beyond the placeholder tile, search, error states, landscape, Dynamic Type, VoiceOver. All need design before build — flag them rather than improvising.

### Accessibility notes for the build
- The wheel needs a non-rotational path: VoiceOver users must be able to reach every list item and control. Expose the underlying list as a standard accessible list and treat the wheel as a supplementary gesture, not the sole input.
- Every wheel hit area is ≥ 56pt; keep all targets ≥ 44pt.
- Amber `#F0A23C` on `#0E1116` clears 4.5:1; iPod blue `#2F6DB5` on `#FBFBF9` clears 4.5:1, and white on `#2F6DB5` clears it in the selection bar. Text at `.35` opacity or below is decorative only — do not lower the contrast of anything meaningful.
- In light theme, blue is load-bearing for selection. Selection must also be conveyed non-chromatically (the bar's fill and the white text do this) so it survives colour-blind and high-contrast modes.

## State Management
Everything is local UI state; no server.

| State | Type | Notes |
|---|---|---|
| `screen` | enum | `menu` · `grid` · `now` · `queue` · `eq`. 1a and 1b hold this separately in the prototype only because both frames are on one page; a real app has one. |
| `selection` | int | Wheel cursor for the current list. Reset to 0 on screen change. |
| `wheelRotation` | float (deg) | Cumulative; drives the ring's visual rotation. Unbounded. |
| `trackIndex` | int | Index into the track list. |
| `position` | int (sec) | Playback position. |
| `playing` | bool | |
| `volume` | float 0–1 | `0.62` at rest. |
| `eqGains` | [int] × 5 | dB, −6…+6. Defaults `[3, 1, −2, 4, 2]`. |
| `eqBand` | int 0–4 | Selected band. |
| `settings` | [bool] × 3 | Gapless / volume limit / shuffle. Defaults `[on, off, on]`. |
| `theme` | enum | `dark` · `light`. Not surfaced in the mocks — no switch is designed yet. Default to following the system appearance, or ask before adding a control. |

Transient, held outside render state: the drag's last angle and the detent accumulator.

Persist across launches: `trackIndex`, `position`, `volume`, `eqGains`, `settings`, `theme`. Real playback should run through `AVAudioEngine`/`AVAudioPlayer` with an `MPNowPlayingInfoCenter` update and remote command handlers so lock screen and CarPlay work.

## Data
Placeholder content, all invented. Six albums (`title`, `artist`, `tag`, `trackCount`) and seven tracks (`title`, `artist`, `duration` in seconds, `albumIndex`). Real implementation reads local file metadata; the `tag` line (`LP · 1998`, `COMP · 1986`) and the `FLAC · 24/96` badge come from container/codec metadata. Durations format as `m:ss`, track numbers zero-pad to two digits.

## Assets
None shipped. Everything is CSS — no images, no icon files.
- **Album artwork**: placeholder colour tiles. Replace with real artwork; keep the frame geometry.
- **Icons**: the 1b tab bar uses coloured shape placeholders; the transport glyphs are typographic (`◀◀ ▶▶ ❙❙ ▶`). Substitute SF Symbols (`backward.fill`, `forward.fill`, `play.fill`, `pause.fill`, `music.note.list`, `square.grid.2x2`, `slider.horizontal.3`) or the codebase's icon set.
- **Fonts**: IBM Plex Sans and JetBrains Mono, both open source (SIL OFL 1.1) — bundle them, or swap to the codebase's equivalents if it already has a type system.

## Files
| File | What it is |
|---|---|
| `iPod Player.dc.html` | The design. Open in a browser; all four frames (2a, 1a, 1b, 1c) are live and interactive. |
| `ios-frame.jsx` | Generic iOS device bezel / status bar used to present the mocks. Not part of the design. |
| `support.js` | Runtime for the design tool's template format. Reference only — do not port. |

Reading the source: `iPod Player.dc.html` holds markup for all three directions plus a logic class at the bottom carrying state, the wheel maths, and the derived values (colours and geometry per row) that the markup consumes.

## Mobile screenshot comparisons

The `screens/` PNGs were exported from 1a (dark) and 2a (light) in `iPod Player.dc.html`. The capture uses the original 402×874 viewport with the mock OS chrome hidden. Now Playing uses the prototype's initial playing state at 1:26. The comparison reads the 1× files, named `{dark,light}-{menu,albums,now-playing,queue,equalizer}.png`. The matching 3× exports use the `@3x.png` suffix.

The artwork bounds in `screens/artwork.json` come from the prototype's visible placeholders. Each screenshot stem has an array of `[left, top, right, bottom]` rectangles in logical pixels, with the right and bottom edges excluded.

Run both commands from `apps/mobile` to render and compare all ten states.

```sh
MYTUNES_RENDER_HANDOFF="$PWD/build/handoff-captures" flutter test test/handoff_capture_test.dart
MYTUNES_HANDOFF_CAPTURES="$PWD/build/handoff-captures" flutter test test/handoff_screenshots_test.dart
```

For a device capture, use the same filenames and match the fixture's library and player state. The comparison accepts an integer pixel ratio and scales to 402×874. Use viewport captures with the device chrome hidden. The report flags missing images, invalid dimensions, and comparisons over the 1.5% threshold. Ordinary `flutter test` runs exercise the renderer and comparator. Setting a capture directory enables the screenshot comparisons.
