# 0019 — E7.S0 Desktop visual mock

- **Status** Pending owner review (2026-11-19)
- **Story** E7.S0

## Design

The desktop library uses the handoff's silver and blue palette in light
mode and ink and amber in dark mode. Transport and the LCD occupy the
top toolbar. The source list and optional album artwork sit beside the
three-column browser and song table. The footer holds library actions,
the library summary, and an appearance control.

The Solid mock supplies sample tracks and local preview state to the
`LibraryWindow` layout. The connected app can supply library and player
state to the same components. The mock renders without IPC. Search,
import, sync, rescan, and device eject belong to later stories.

The `desktop` group in `design/tokens.json` adds the following CSS roles.

- Surfaces and color roles `chromeTop`, `chromeBottom`, `sidebar`,
  `browser`, `canvas`, `stripe`, `header`, `edge`, `highlight`, `secondary`,
  `softAccent`, `lcdTop`, `lcdBottom`, `lcdText`, `lcdSub`, `controlTop`,
  `controlBottom`, and `controlEdge`.
- Spacing roles `tight`, `small`, `medium`, `large`, `gutter`, and `section`.
- Radius roles `control` and `panel`.
- Size roles `toolbar`, `sidebar`, `browser`, `footer`, `row`,
  `columnHeader`, `item`, `facetItem`, `icon`, `iconLarge`, `control`,
  `play`, `progress`, `search`, `lcd`, `volume`, `art`, and `trackCheck`.
- Typography roles `body`, `strong`, `title`, `label`, `small`, `mono`,
  and `section`, using the existing IBM Plex Sans and JetBrains Mono
  families. Local WOFF2 files load without a network request.

## Review

Run `pnpm dev:mock` from `apps/desktop` and open the printed URL with
`?theme=light` or `?theme=dark`. The gear in the footer also switches
between themes.

The [light screenshot](../../design/desktop/light.png) and
[dark screenshot](../../design/desktop/dark.png) were captured at
1440 × 900 in WebKitGTK. Both themes were also checked in WKWebView on
macOS. Preview checks covered Browse, transport, artwork visibility,
and the theme control. Owner approval is pending.

## Sample artwork

The cover at `apps/desktop/src/mock/aerial-lines.webp` was generated
with the built-in image tool using this prompt.

> Use case: photorealistic-natural. Asset: one original square independent electronic music album cover for a desktop music-library mock. An isolated concrete coastal listening station built into a rugged cliff at dusk, looking out across the slate-blue sea toward a small warm amber sun. Frame the low geometric building near the center so the image reads as a tiny thumbnail. Analog 35 mm editorial photograph, muted blue-gray and rust tones, fine natural film grain, restrained contrast. Full bleed. No people, lettering, logos, or watermark.
