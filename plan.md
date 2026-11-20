# MyTunes: a retro click-wheel player for mobile and a desktop library app

> **Who this is for:** an implementing agent (or human) building this project from an empty repo.
> Work through it **milestone by milestone, in order**. Each story has an ID (`E3.S2`) and acceptance
> criteria (AC). A story is done only when every AC passes **and** the Definition of Done (§9) is met.
> Tick the checkbox next to a story when it's done, and record any deviation in `docs/decisions/` (§10).

---

## 0. Product summary

Two apps that work together, the way a click-wheel iPod worked with its desktop jukebox in 2005:

| App | Role | Platforms |
|---|---|---|
| **MyTunes Pod** (mobile) | Plays music stored on the phone. The primary UI is a click wheel: drag around the ring to move through lists, press the centre to choose. It holds **only what the desktop synced to it**. | iOS 16+, Android 8+ (API 26) |
| **MyTunes** (desktop) | Owns the music library: import, organize, tag, playlists, playback. Syncs a chosen subset to paired phones over Wi-Fi. It is the source of truth. | Linux (primary dev target), macOS, Windows |

**Distribution:** an open-source **public GitHub repository**. Releases go out as GitHub Release artifacts. **No app-store submissions** (no App Store, no Play Store). See §7 R1 and E15.

**Principles**
1. **Local files only.** No streaming, no accounts, no cloud, no DRM content. The user's own music files.
2. **Desktop is the master.** The phone is a "device" the desktop fills. The phone never edits the library. It sends back only play counts, skips, ratings and On-The-Go playlists, the same things an iPod did.
3. **Retro feel, modern robustness.** Detented wheel, haptic clicks, a "Do not disconnect" sync screen. Underneath: atomic sync, mutual TLS, crash-safe databases.
4. **One audio engine, one data model.** The Rust core is shared by both apps, so playback (gapless, EQ, volume normalization) sounds the same everywhere.

**Non-goals for v1:** music store, streaming, podcasts, video, CD ripping, cloud backup, multiple desktop libraries per phone, Apple Music or Spotify integration, CarPlay or Android Auto, app-store distribution.

---

## 1. Design inputs

- **Mobile design (final intent, high fidelity):** `design/handoff/README.md`. **Read it in full before starting any mobile UI story.** It specifies tokens, type, spacing, every screen, and the wheel maths. `design/handoff/iPod Player.dc.html` is the interactive prototype. Source: the owner's Claude Design canvas, <https://claude.ai/artifact/G6b9ykA3mZ7rUDFSFxcAos>.
  - `design/handoff/support.js` and `ios-frame.jsx` are the design tool's runtime and an OS mock. They are **gitignored, not committed** (third-party files in a public repo). The prototype needs them to run, so keep local copies or open the canvas link to view it interactively. Commit reference **screenshots** of every screen in both themes to `design/handoff/screens/` instead (story E0.S3); golden tests compare against those.
- **Chosen direction (confirmed by owner):** **1a "Wheel-first"** (dark, amber accent) and **2a "Light iPod"** (white body, silver wheel, blue selection bar), built as **one themed component tree with two colour sets**. Directions **1b (modern tabs)** and **1c (hybrid scrub arc)** are **not in v1**. They're in the backlog (§8).
- **Do not port** `support.js` or `ios-frame.jsx`. Don't draw a status bar, device bezel or home indicator. The five review tabs under the 1a frame are **not part of the app**.
- **Desktop design:** there's no mock yet. E7 gives a structural spec (the classic 2005-era jukebox layout) reinterpreted with the same token families. The desktop UI is HTML/CSS, so **the mock is built as a static Solid page in the real app shell** (story E7.S0) and approved by the owner before the rest of E7.
- **Design gaps.** The handoff explicitly does not cover: empty library, import, loading and buffering, missing artwork, search, errors, landscape, Dynamic Type, VoiceOver, Artists/Genres/Playlists/Songs list screens, Settings, pairing and the sync screen. **Rule: never improvise new visual language.** Extend only by recombining existing components and tokens (Menu row, Queue row, header eyebrow, status row). Any screen marked `🎨 design-gate` needs an owner-approved mock or screenshot before merge.

---

## 2. Tech stack

### 2.1 Decision summary

| Layer | Choice | Why |
|---|---|---|
| Shared core | **Rust** workspace (`crates/*`) | One implementation of the library DB, tag reading, audio engine, and sync protocol (both client and server ends) for both apps. |
| Desktop shell | **Tauri 2** (2.12.x at time of writing) | The Rust core links directly into the Tauri backend process. Small binaries. It uses the system webview: WebKitGTK on Linux, WKWebView on macOS, WebView2 on Windows. |
| Desktop UI | **SolidJS 2.0** + TypeScript (strict) + **Vite** | Fine-grained reactivity with no virtual DOM, so very large tables stay cheap to update. Chosen by the owner. |
| Desktop IPC typing | **tauri-specta** (2.0 RC) + **specta** | Generates `bindings.ts` from Rust command and event signatures, so the frontend and backend can't drift apart. |
| Mobile UI | **Flutter** (Dart, stable channel) | Every pixel of the iPod UI is custom-drawn, which suits Flutter well. One codebase for iOS and Android, with low-latency touch and haptics. |
| Mobile ↔ Rust | **flutter_rust_bridge v2** (with cargokit) | Generated, type-safe bindings. Async, plus `Stream`s for position and state events. |
| Audio decode | **symphonia** | Pure Rust. MP3, AAC-LC (M4A), ALAC, FLAC, WAV/AIFF, Ogg Vorbis. Opus is via the `opus` crate (libopus) if needed. |
| Audio output | **cpal** (CoreAudio on macOS/iOS, AAudio on Android, WASAPI on Windows, PipeWire/ALSA on Linux), **rubato** for resampling | The same engine everywhere. On desktop it runs in the Rust process, **not in the webview** (no `<audio>` element), so EQ, gapless and background playback don't depend on the webview. |
| DSP | Hand-written biquad 5-band EQ, ReplayGain ("Sound Check"), soft limiter | ~200 lines. Identical on every platform. |
| Tags and art | **lofty** (read and write), **image** (resize and encode thumbnails) | Covers ID3v2, Vorbis comments, MP4 atoms, APE. |
| Database | **SQLite** via **rusqlite** (`bundled` feature), hand-rolled migrations | Desktop and phone use separate DBs with shared schema crates. |
| Sync transport | **axum** + **rustls** server (desktop, inside the Tauri process), **reqwest** with rustls client (phone, in Rust), **rcgen** self-signed certs, **blake3** hashes | Mutual TLS with pinned certs. Both ends are Rust, so neither the webview nor Dart does any TLS. |
| Discovery | Desktop advertises with **mdns-sd**. Phone discovers with Flutter **bonsoir** (native Bonjour/NSD). | iOS forbids raw multicast without a special entitlement, but native Bonjour is allowed. |
| Desktop OS integration | **souvlaki** (MPRIS, macOS Now Playing, Windows SMTC, media keys), **notify** (watch folders), Tauri plugins: `dialog`, `window-state`, `single-instance`, `updater`, `log`, `opener` | No `shell` or `fs` plugin. All file access happens in Rust. |
| Mobile OS integration | **audio_service** (lock screen, notification, remote commands, Android foreground service), **audio_session** (AVAudioSession category, interruptions, unplugging headphones pauses playback), **wakelock_plus** (during sync), **mobile_scanner** (pairing QR), **flutter_secure_storage** | These plugins handle the OS media-session side. Rust only produces the audio. |
| Mobile state | **riverpod** | |
| Fonts | IBM Plex Sans and JetBrains Mono (SIL OFL), **bundled locally** in both apps as woff2 or ttf | Per the handoff. No font CDNs, so the apps work offline and don't leak requests. |
| Frontend tooling | **pnpm**, **@solidjs/vite-plugin**, Vitest + `@solidjs/testing-library` (1.0 RC for Solid 2), Biome, `tsc --noEmit` (TypeScript 7) | Exact versions in ADR-0001. |
| CI | GitHub Actions: Linux runner (Rust, Android, Linux desktop), macOS runner (iOS, macOS desktop), Windows runner (Windows desktop) | **iOS builds need macOS with Xcode.** The dev machine is Linux, so iOS builds and testing go through CI, a Mac, or a cloud Mac. |

### 2.2 Shape of the system

```
┌──────────────────────────────────────────┐        ┌──────────────────────┐
│ Desktop (Tauri 2)                        │        │ Mobile UI (Flutter)  │
│  ┌────────────────────────────────────┐  │        └──────────┬───────────┘
│  │ Webview: SolidJS 2.0 UI (TS)       │  │                   │ flutter_rust_bridge
│  └──────────────┬─────────────────────┘  │                   │
│     commands / Channels / mtart:// art   │                   │
│  ┌──────────────┴─────────────────────┐  │                   │
│  │ src-tauri: thin command layer      │  │                   │
│  └──────────────┬─────────────────────┘  │                   │
└─────────────────┼────────────────────────┘                   │
┌─────────────────┴────────────────────────────────────────────┴───┐
│  Rust core: mt-db · mt-meta · mt-audio · mt-smart · mt-sync-*    │
│             mt-app (desktop use cases)   mt-ffi (phone facade)   │
└──────────────────────────────────────────────────────────────────┘
```

The two UIs share **no screens** (a desktop jukebox window and an iPod are completely different), so using two UI stacks costs little. Everything worth sharing is in the Rust core. The only shared design asset is `design/tokens.json`, which generates both Dart and CSS.

### 2.3 Risks and how the plan handles them
- **Solid 2.0 is brand new.** 2.0.0 was released 2026-11-17, and some libraries haven't caught up: `vite-plugin-solid` has been replaced by `@solidjs/vite-plugin` (3.0.1, stable since 2026-11-17), but `@tanstack/solid-virtual` still declares Solid 1.x. Mitigations:
  - Pin exact versions in `package.json` and commit the lockfile.
  - Add a dependency only if its `peerDependencies` accept `solid-js@2`.
  - **Build the virtualized track table ourselves** (fixed row height, ~300 lines; E7.S3) rather than depending on a 1.x library.
  - Avoid heavy UI component kits. Hand-build components from tokens.
  - **Most online examples and model training data are Solid 1.x.** The implementing agent must read the Solid 2.0 docs and migration guide before writing components, and record the idioms used in ADR-0005.
  - **Fallback:** Solid 1.9.x (`v1-latest`) if the spike hits a blocker.
- **WebKitGTK on Linux.** Linux is the primary platform and its webview is the slowest and quirkiest of the three. Known problem: blank or flickering windows on some NVIDIA + Wayland setups. The workaround is `WEBKIT_DISABLE_DMABUF_RENDERER=1`; document it and set it automatically when NVIDIA is detected. Spike S1 measures table performance **on WebKitGTK specifically**.
- **Three different webview engines.** CSS must work on WebKit (macOS and Linux) and Chromium (Windows WebView2). Avoid bleeding-edge CSS without fallbacks. CI runs E2E on Linux and Windows.
- **Rust audio on mobile.** Background playback, interruptions and route changes are normally the native player's job. Mitigation: spike S2 in M0. **Fallback:** `just_audio` (native AVQueuePlayer/ExoPlayer) for output, with EQ dropped on iOS (just_audio offers no EQ on iOS). The handoff requires EQ, which is why Rust audio is plan A.
- **iOS builds on a Linux workstation.** Mitigation: a macOS CI job from M0. Device installs need Xcode on a Mac, or a sideloading tool (E15.S2).

### 2.4 Alternatives considered (short)
- **Tauri 2 mobile with Solid for the phone too:** one frontend language for both apps. Rejected because webview touch-to-haptic latency and background audio on mobile webviews are weaker, and the wheel is the load-bearing interaction.
- **GPUI desktop:** the original proposal. The owner preferred Tauri + Solid. A web UI also gives better accessibility, since webviews expose a real accessibility tree to screen readers.
- **All Flutter, including desktop:** one UI toolkit, but a weaker dense-table and desktop feel.

---

## 3. Architecture

### 3.1 Repository layout
```
mytunes/
├─ plan.md                      ← this file
├─ LICENSE, README.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md
├─ Cargo.toml                   ← Rust workspace (includes apps/desktop/src-tauri)
├─ crates/
│  ├─ mt-types/                 ← IDs, Track/Album/Playlist structs, errors, serde + specta derives
│  ├─ mt-db/                    ← SQLite schemas + migrations (desktop & device), query layer
│  ├─ mt-meta/                  ← scan, tag read/write (lofty), artwork extract/resize, codec info
│  ├─ mt-audio/                 ← decoder, gapless queue, EQ, ReplayGain, output, ui-sfx channel
│  ├─ mt-smart/                 ← smart playlist rule model + SQL compiler
│  ├─ mt-sync-proto/            ← wire types, versioning, manifest/diff logic (pure, no IO)
│  ├─ mt-sync-server/           ← desktop: axum + rustls + mdns-sd + pairing
│  ├─ mt-sync-client/           ← phone: reqwest client, downloader, atomic commit
│  ├─ mt-app/                   ← desktop use cases (library, views, player, devices); no Tauri types; unit-testable
│  └─ mt-ffi/                   ← flutter_rust_bridge API for the phone (thin facade)
├─ apps/
│  ├─ desktop/
│  │  ├─ src-tauri/             ← Tauri backend: commands, channels, mtart:// protocol, plugins, capabilities
│  │  ├─ src/                   ← SolidJS 2.0 frontend (TS)
│  │  │  ├─ bindings.ts         ← GENERATED by tauri-specta (do not edit)
│  │  │  ├─ theme/tokens.css    ← GENERATED from design/tokens.json
│  │  │  ├─ components/         ← VirtualTable, SourceList, LcdPanel, Transport, …
│  │  │  └─ state/              ← stores, IPC adapters
│  │  ├─ index.html, vite.config.ts, package.json
│  │  └─ e2e/                   ← WebdriverIO + tauri-driver tests
│  └─ mobile/                   ← Flutter app (rust_builder/ via cargokit)
├─ design/
│  ├─ handoff/                  ← README.md + iPod Player.dc.html (runtime files gitignored) + screens/
│  ├─ tokens.json               ← colour/type/spacing tokens, both themes
│  └─ desktop/                  ← approved desktop mock screenshots (E7.S0)
├─ fixtures/audio/              ← generated test tones only (never copyrighted music)
├─ packaging/                   ← AUR PKGBUILD, Flatpak manifest (later), icons
├─ tools/                       ← token codegen, fixture generator scripts
└─ docs/
   ├─ decisions/                ← ADRs
   └─ sync-protocol.md
```

### 3.2 Data model (desktop library DB, `library.sqlite`)
```
tracks(rowid INTEGER PK /*internal FTS key*/, id TEXT UNIQUE NOT NULL /*uuid v7*/,
       file_path TEXT UNIQUE, file_rev INT, file_hash BLOB /*blake3*/,
       size_bytes INT, codec TEXT, sample_rate INT, bit_depth INT, bitrate INT, channels INT,
       duration_ms INT, title, artist, album_artist, album, composer, genre, year INT,
       track_no INT, track_total INT, disc_no INT, disc_total INT, compilation BOOL,
       sort_title, sort_artist, sort_album, sort_album_artist,
       rating INT /*0-100 step 20*/, rating_updated_at INT,
       play_count INT, skip_count INT, last_played_at INT, date_added INT, date_modified INT,
       replaygain_track_db REAL, replaygain_album_db REAL, artwork_id TEXT NULL,
       checked BOOL DEFAULT 1, missing BOOL DEFAULT 0)
albums  — derived view (album_artist|album|year) or materialized table for speed
artwork(id TEXT PK /*blake3 of source bytes*/, thumb_300 BLOB, thumb_600 BLOB)
playlists(id PK, name, kind /*manual|smart|folder|otg*/, parent_id, smart_rules JSON, sort_order)
playlist_items(playlist_id, position, track_id)
devices(id PK, name, model, platform, cert_fingerprint, paired_at, last_sync_at,
        capacity_bytes, free_bytes, sync_mode /*all|selected|manual*/, sync_selection JSON,
        reserve_free_bytes INT)
device_tracks(device_id, track_id, file_rev, synced_at)          — what we believe is on it
applied_events(device_id, event_id) — idempotency for play-stat merges
settings(key PK, value JSON)
```
**Device DB** (`device.sqlite`, on the phone) is a subset: `tracks` (metadata only, with a local `rel_path` instead of the desktop path), `albums`, `artwork` (thumbs only), `playlists` + `playlist_items`, `events` (play/skip/rating/otg log, with `event_id` uuid, `ts`, `acked BOOL`), `state` (resume position, EQ, settings, theme).

**Key rule:** the phone **never reads tags from files**. All metadata arrives from the desktop in the sync manifest. A tag edit on desktop therefore only needs a metadata update, not a file re-send. `file_rev` increments only when the audio file itself is replaced.

### 3.3 Sync protocol (summary; full spec goes in `docs/sync-protocol.md`, story E8.S1)
- **Discovery:** desktop advertises `_mytunes._tcp` with TXT `{v=1, lib=<library_id>, name=<desktop name>}`. The phone browses with bonsoir. The pairing QR also carries IP:port candidates as a fallback.
- **Pairing (once):** desktop shows a QR with `{library_id, host candidates, port, server_cert_sha256, one_time_token (expires 5 min)}` plus the same info as a 6-word code. The phone generates its own key pair and cert, then calls `POST /v1/pair {token, device_name, model, platform, client_cert}`. Both sides persist the peer fingerprint. After that, every request is **mutual TLS with pinned fingerprints**. Unpaired clients get only `/v1/pair`, which is rate-limited (5 attempts/min) and disabled unless the desktop pairing sheet is open.
- **Session:**
  1. `POST /v1/sync/begin {device_state: {free_bytes, capacity_bytes, db_schema_v, have: [{track_id, file_rev}], last_manifest_id}}` returns `{session_id}`.
  2. `POST /v1/sync/events {events: [...]}` uploads unacked play/skip/rating/OTG events. The desktop applies them idempotently and returns acked ids.
  3. `GET /v1/sync/{session}/plan` returns `{add: [{track_id, file_rev, size, hash}], remove: [track_id], metadata: <full device DB snapshot or delta>, artwork: [ids], bytes_total}`. The plan respects the device's sync selection and `free_bytes - reserve`.
  4. `GET /v1/files/{track_id}?rev=` supports HTTP `Range` (resumable). The phone writes to `tmp/`, verifies blake3, then renames.
  5. `GET /v1/artwork/{id}/{300|600}`.
  6. Phone applies metadata, removals and the new playlist set **in one SQLite transaction**, then deletes orphaned files, then calls `POST /v1/sync/{session}/commit {manifest_id}`.
- **Merge semantics:** play events add to `play_count` and set `last_played_at = max`. Skip events increment `skip_count`. Ratings are last-writer-wins by `rating_updated_at`. OTG playlists become desktop playlists named `On-The-Go N` (desktop-side dedupe by OTG uuid).
- **Failure:** at every step before commit, the phone's visible library is unchanged. Re-running sync resumes partial downloads.
- **Versioning:** `/v1/hello` exchanges `{protocol_v, min_supported}`. A mismatch shows "Update MyTunes" on whichever side is older.

### 3.4 Audio engine (`mt-audio`)
- Decoder thread (symphonia) feeds a lock-free ring buffer that the output callback (cpal) drains. The next track is pre-decoded ≥ 10 s before the current one ends, for **sample-accurate gapless** playback (honours encoder delay/padding from LAME/iTunSMPB headers).
- DSP chain: ReplayGain (track or album, if enabled) → 5-band peaking EQ (60, 230, 910, 3.6k, 14k Hz, Q≈1.0, ±6 dB, matching the handoff's ±6 range) → volume → volume limit (cap) → soft limiter.
- Resample to the device rate with rubato when it differs. Output format is f32.
- **UI-sfx channel:** a synthesized wheel click mixes into the next output callback on desktop and Android. iOS uses System Sound Services for UI sound behavior. See ADR-0018.
- API: `load_queue(items, start_index)`, `play/pause/toggle`, `seek(ms)`, `next/prev` (prev restarts if > 4 s elapsed, per the handoff), `set_eq(gains)`, `set_volume`, `set_flags(gapless, sound_check, volume_limit)`, `events() -> Stream<PlayerEvent{state, position_ms, track_id, ended, error}>`.
- Mobile: Rust owns the cpal stream. Dart owns the OS media session (`audio_service`) and the audio session (`audio_session`), and forwards remote commands and interruptions to Rust.
- Desktop: the player lives in the Tauri process's managed state. The webview only sends commands and renders events.

### 3.5 Desktop IPC design (Tauri 2)
- **Commands** are thin `#[tauri::command]` wrappers over `mt-app` functions, annotated with `#[specta::specta]`. `tauri-specta` exports `apps/desktop/src/bindings.ts` on debug builds. CI fails if it's stale.
- **Streams** use `tauri::ipc::Channel<T>`, passed by the frontend into a subscribe command: player events (state change immediately, position at 4 Hz), scan progress, sync progress. **Global notifications** (`LibraryChanged{revision}`, `DeviceOnline{id}`) are typed tauri-specta events.
- **Big lists never cross IPC whole.** For each view (library, playlist, search, browser filter) the backend materializes an ordered `Vec<TrackId>` snapshot keyed by `view_id` + `revision`. The frontend calls `view_open(spec) -> {view_id, count, revision}` and then `view_rows(view_id, offset, limit=200) -> Row[]`. The frontend keeps an LRU of 200-row pages. Sorting or filtering creates a new snapshot. `LibraryChanged` invalidates it.
- **Artwork** is served by an async custom URI scheme `mtart` (`mtart://localhost/<id>/<300|600>` on macOS/Linux, `http://mtart.localhost/...` on Windows; wrap the difference in one TS helper) with `Cache-Control: immutable`. Images never go through IPC as base64.
- **Security:** Tauri v2 capabilities allow only our own commands plus the needed plugin permissions. Strict CSP: `default-src 'self'; img-src 'self' mtart: http://mtart.localhost data:; style-src 'self'; connect-src ipc: http://ipc.localhost`. No remote content is ever loaded into the webview.

### 3.6 Design tokens
`design/tokens.json` holds both colour sets (`dark`, `light`), type roles, spacing, radii and shadows, transcribed **exactly** from `design/handoff/README.md`. `tools/gen_tokens` emits:
- `apps/mobile/lib/theme/tokens.g.dart`, and
- `apps/desktop/src/theme/tokens.css`: CSS custom properties under `:root` (light) and `[data-theme="dark"]` / `prefers-color-scheme: dark` (dark).

Hand-editing generated files is forbidden.

---

## 4. Milestones overview

| # | Milestone | Outcome (exit criteria) |
|---|---|---|
| **M0** | Foundations & spikes | The public repo and CI on 3 OSes plus iOS and Android builds are green. Spikes S1–S3 are each documented in an ADR with a **go/no-go** decision. |
| **M1** | Core library & audio | `mt-db`, `mt-meta`, `mt-audio` pass tests. The CLI `mt-cli` can scan a folder and play gaplessly with EQ on Linux. |
| **M2** | Desktop MVP | A user can import folders, browse and sort a track table, search, and play music in the Tauri app. Media keys work. |
| **M3** | Mobile MVP (wheel) | On iOS and Android, the 1a and 2a themes are pixel-faithful. The wheel navigates Menu, Albums, Now Playing, Queue and EQ over a dev-seeded library, with background and lock-screen playback. |
| **M4** | Sync v1 | Pair over QR, then Wi-Fi sync of the "Entire library" or "Selected playlists" with a capacity check. Play counts flow back. Atomic and resumable. |
| **M5** | Library depth | Desktop: playlists, smart playlists, Get Info tag editing, artwork, organize-files. Mobile: full iPod menu tree, shuffle/repeat, On-The-Go playlist, Settings. |
| **M6** | Retro polish & accessibility | Clicker sound, the "Do not disconnect" screen, capacity bar, Sound Check, EQ presets, VoiceOver/TalkBack, Dynamic Type, empty and error states. |
| **M7** | Release | Tagged GitHub Release with desktop installers (AppImage, deb, rpm, AUR, dmg, msi/nsis) plus in-app updater, a signed Android APK, an unsigned iOS IPA with sideload docs, and user and dev docs. |

Dependencies: M0 → M1 → (M2 ∥ M3) → M4 → M5 → M6 → M7. M2 and M3 can run in parallel once M1 is done.

---

## 5. Epics, stories and acceptance criteria

Notation: **As a** *role*, **I want** *capability*, **so that** *benefit*. ACs are testable statements.
`🎨 design-gate` = needs an owner-approved mock before merge. `🧪` = must have an automated test.

---

### M0 — Foundations & spikes

#### E0 Repository & toolchain
- [x] **E0.S1 Monorepo skeleton.** As a developer, I want the workspace laid out per §3.1 so that every later story has a home.
  - AC1: `cargo build --workspace` and `cargo test --workspace` succeed on Linux with empty crates.
  - AC2: `apps/mobile` is a Flutter app that builds for Android (`flutter build apk --debug`) and links `mt-ffi` through flutter_rust_bridge. A Dart call `ffi.ping()` returns `"pong"` from Rust in an integration test 🧪.
  - AC3: `apps/desktop` runs with `pnpm tauri dev` on Linux and shows a Solid 2.0 page titled "MyTunes". A button calls a tauri-specta-typed `ping` command and renders `"pong"` 🧪 (Vitest with `mockIPC`).
  - AC4: Versions pinned. `rust-toolchain.toml` pins Rust. Flutter is pinned via `.fvmrc` or documented. Node is pinned via `.nvmrc` and `packageManager: pnpm@…`. Exact versions (no `^`/`~`) for `solid-js`, `@solidjs/web`, `vite-plugin-solid`, `@tauri-apps/*` and the `tauri*` crates. The lockfiles are committed. All of this is recorded in ADR-0001.
  - AC5: `README.md` documents setup on Linux (including Arch/CachyOS package names): `webkit2gtk-4.1`, `libayatana-appindicator` (if used), Android SDK/NDK, `cargo-ndk`, pnpm. It also covers how to build iOS (macOS only) and the NVIDIA/Wayland `WEBKIT_DISABLE_DMABUF_RENDERER` note.
- [ ] **E0.S2 CI.** As the owner, I want every PR verified on all targets so that platform breakage is caught early.
  - AC1: GitHub Actions jobs:
    - `rust`: fmt, clippy `-D warnings`, test, on ubuntu, macos and windows.
    - `desktop`: `pnpm install --frozen-lockfile`, typecheck, lint, Vitest, `tauri build --debug`, on all three.
    - `android`: debug APK.
    - `ios`: on macos, `flutter build ios --no-codesign`.
    - `flutter`: analyze, test.
    - `licenses`: `cargo deny check`, plus an npm licence check.
  - AC2: Cargo, pnpm and pub caches are used. A full run takes < 25 min.
  - AC3: The Linux desktop AppImage and the Android debug APK are uploaded as run artifacts.
  - AC4: Workflows triggered by fork PRs have no access to secrets. Release signing secrets are used only on tag pushes from `main` (E15).
  - _Status 2026-11-18: `.github/workflows/ci.yml` written (plus a Linux E2E smoke test via tauri-driver). Every step passes locally on Linux. Not yet run on GitHub, so it stays unticked until the first green run, including the macOS, Windows and iOS jobs._
  - _Status 2026-11-18 (later): the E0 work is now committed locally on `main` (first commit `0985e93`, "E0: baseline"); the repo had no commits before that. Still to do: add the GitHub remote, push `main`, and get the first green CI run._
- [ ] **E0.S3 Design tokens & reference screens.** As a developer, I want tokens in one file with codegen, and committed reference screenshots, so that both apps use exact design values.
  - AC1: `design/tokens.json` contains every colour, type role, spacing, radius and shadow from `design/handoff/README.md` §Design Tokens, for `dark` and `light`, named after the handoff's token names.
  - AC2: `tools/gen_tokens` produces Dart and CSS outputs. CI fails if the generated files are stale 🧪.
  - AC3: A unit test asserts sample values (`amber == #F0A23C`, `light.accent == #2F6DB5`, `wheel diameter == 252`) 🧪.
  - AC4: PNG screenshots of every 1a and 2a screen (Menu, Albums, Now Playing, Queue, Equalizer) at 402×874 @1× and @3× are captured from the prototype into `design/handoff/screens/`. `.gitignore` excludes `design/handoff/support.js` and `design/handoff/ios-frame.jsx`.
  - _Status 2026-11-18: AC1–AC3 done (the Dart output carries `// dart format off` so formatters can't make it stale). AC4 is pending: the screenshots still need capturing from the prototype. The gitignore part is done._
- [x] **E0.S4 Test fixtures.** As a developer, I want generated audio fixtures so that tests never depend on copyrighted music.
  - AC1: `tools/gen_fixtures` (using sox/ffmpeg if present, otherwise pure-Rust WAV + FLAC encoding) generates sine/chirp files in MP3, M4A (AAC), ALAC, FLAC 16/44.1 and 24/96, WAV, Ogg, with varied tags (including Unicode, missing fields, embedded art, compilation flag).
  - AC2: It includes a **gapless pair**: one continuous sine split into two files at a non-zero-crossing point, for the gapless test.
  - AC3: Fixtures are generated in CI, not committed (except a tiny `.wav` smoke file).
- [x] **E0.S5 Open-source repo hygiene.** As a contributor, I want a well-run public repo so that I can build, contribute and report issues safely.
  - AC1: Dual licence **`MIT OR Apache-2.0`** (§7 Q5). `LICENSE-MIT` and `LICENSE-APACHE` contain the canonical texts (copy from the official SPDX or Apache sources, not from memory). The README has a "License" section with the standard Rust dual-licence contribution clause. `license = "MIT OR Apache-2.0"` is set in `[workspace.package]`, and `package.json` and `pubspec.yaml` carry the same SPDX expression.
  - AC2: `README.md` covers what it is, screenshots (once available), build-from-source for each platform, and links to releases. `CONTRIBUTING.md` covers dev setup, the plan/ADR process, commit convention (`E6.S1: …`) and a "never commit copyrighted audio" rule. Also `CODE_OF_CONDUCT.md`, issue templates (bug, feature) and a PR template with a DoD checklist.
  - AC3: `SECURITY.md` explains how to privately report vulnerabilities (the sync server is network-exposed), via GitHub private vulnerability reporting.
  - AC4: `cargo-deny` config allows only these licences: MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, and OFL-1.1 for fonts. GPL, LGPL and AGPL are denied, and so is anything unlicensed. The npm licence check uses the same allow-list. Duplicate crypto crates are banned. Dependabot or Renovate is enabled for cargo, npm, pub and GitHub Actions.
  - AC5: The repo name, description and topics avoid Apple trademarks ("click-wheel music player", "retro jukebox"). See §7 R1.

#### E1 Technical spikes (time-boxed: 2–3 days each, output = ADR + throwaway branch)
- [ ] **E1.S1 Spike: Tauri + Solid 2.0 track table at scale.** As the owner, I want proof the webview desktop can render a jukebox-grade table so that we commit to it with confidence.
  - AC1: A demo window uses a **hand-rolled virtualized table** (fixed 22 px rows, 8 columns, sortable header, resizable columns, shift and ctrl selection) backed by the §3.5 `view_open`/`view_rows` paging over 50,000 synthetic rows in SQLite.
  - AC2: **On WebKitGTK (Linux)**, scrolling holds ≥ 55 fps (measured with the WebKit inspector timeline or a rAF frame counter). A page fetch round-trip (`view_rows`, 200 rows) completes in < 8 ms. A sort of 50k rows refreshes the visible rows in < 150 ms. The same is measured on WebView2 (Windows) and WKWebView (macOS) via CI or manually.
  - AC3: Dragging files from the OS file manager onto the window delivers absolute paths via Tauri's drag-drop event.
  - AC4: A `Channel` streaming 4 Hz position events for 10 min shows no memory growth in the webview.
  - AC5: The Solid 2.0 toolchain works end to end: dev server with HMR, production build, Vitest component test with `@solidjs/testing-library`. ADR-0005 lists the Solid 2.0 idioms the codebase will use (reactivity primitives, async data, error and loading boundaries, stores), taken from the official 2.0 docs.
  - AC6: ADR-0002 records go/no-go and gaps found. **Solid 2.0 blocker → fall back to Solid 1.9.x. Webview performance blocker → canvas-rendered table body (keep DOM for header and chrome).**
  - _Status 2026-11-18: **GO** (ADR-0002) — no fallback needed. Spike code lives on throwaway branch `spike/e1-s1-table` (do not merge; re-use as the E7.S4 reference implementation). WebKitGTK (Linux/Wayland, AMD GPU, 60 Hz, production frontend): page fetch median 2 ms (max 3; bar < 8), sort→visible refresh 29/48/32 ms (bar < 150), scrolling 60.2 avg / 55.6 1% low fps at 200 px/s and 60.0 / 55.6 at 1200 px/s, jumps 62.3 avg (bar ≥ 55; 60 fps is the display ceiling). AC1 built: hand-rolled pool-rendered `VirtualTable`, 22 px rows, 8 sortable + resizable columns, shift/ctrl selection, SQLite-backed `view_open`/`view_rows` paging over 50k rows. AC4: 10-min 4 Hz `Channel` stream, no RSS growth (see ADR-0002 for numbers). AC5: dev server + HMR verified in a real engine, `tsc` + production build green, 6 Vitest tests with `@solidjs/testing-library`, tauri-driver E2E smoke green. AC3 (drag-drop) is wired and compiled but still needs **one physical drag** onto the window by the owner; AC2's WebView2/WKWebView numbers are pending the first CI run (or a manual `MT_SPIKE_AUTORUN=full MT_SPIKE_RESULTS=<path>` run — exact steps in ADR-0002 "Gaps"). Learnings for later stories are in ADR-0005 (Solid 2.0 idioms: staged writes/flush, split effects, ref-callback cleanup rule, tauri-specta mock double-wrap gotcha)._
- [ ] **E1.S2 Spike: Rust audio on mobile.** As the owner, I want proof that the Rust engine survives mobile lifecycles so that EQ and gapless can be identical everywhere.
  - AC1: On a physical iPhone and a physical Android phone, a Flutter test app plays a FLAC through Rust/cpal.
  - AC2: Playback continues with the screen locked for 10 minutes. Lock-screen and notification play/pause/next work via audio_service.
  - AC3: An incoming call or alarm pauses playback. It resumes (or doesn't) per OS convention. Unplugging wired or Bluetooth headphones **pauses**.
  - AC4: Measured gap between the gapless-pair fixture tracks is 0 samples (verified by capturing output, or by asserting on the engine's sample counter).
  - AC5: ADR-0003 records go/no-go. **No-go → plan B: `just_audio` output, EQ Android-only, document the iOS EQ gap.**
- [ ] **E1.S3 Spike: Wi-Fi sync handshake on iOS.** As the owner, I want proof that iOS allows LAN discovery and mutual-TLS transfer from our app so that the sync design holds.
  - AC1: An iPhone discovers the desktop's `_mytunes._tcp` via bonsoir, with the local-network permission prompt shown (Info.plist `NSLocalNetworkUsageDescription`, `NSBonjourServices`).
  - AC2: The Rust client on the phone completes mutual TLS with pinned self-signed certs against a server running **inside the Tauri process**, and downloads a 200 MB file at ≥ 10 MB/s on 5 GHz Wi-Fi.
  - AC3: Behaviour is documented for backgrounding the app mid-transfer (expected: the transfer pauses, then resumes via Range on return).
  - AC4: It works with a **free Apple ID** development profile (no paid account), confirming the sideload path in E15.S2.
  - AC5: ADR-0004 written.

**M0 exit:** E0 complete, the repo public on GitHub, four ADRs (0002–0005) with decisions, plan §2 updated if any spike failed.

_2026-11-19: The owner started M1 while the platform checks remain open. Remaining spike evidence includes Windows WebView2, headphone disconnect on Android, and signed physical-iPhone audio and sync runs. The spike harnesses stay on their own branches. See ADR-0007._

---

### M1 — Core library & audio

#### E2 Library database (`mt-db`)
- [ ] **E2.S1 Desktop schema & migrations.** As a developer, I want a versioned schema so that upgrades never lose a library.
  - AC1: The schema in §3.2 is created by migration `V1`. `PRAGMA user_version` tracks the version. Migrations run in a transaction 🧪.
  - AC2: WAL mode. `foreign_keys=ON`. FTS5 virtual table over title/artist/album/album_artist/composer/genre, kept in sync with triggers 🧪.
  - AC3: Opening a DB from a newer schema fails with a typed `SchemaTooNew` error (no silent downgrade) 🧪.
  - _Status 2026-11-19: V1 implemented with bundled SQLite. Integration tests cover fresh creation, reopening, rollback, WAL and foreign keys, FTS updates and newer-version rejection. Local verification is complete. CI is pending._
- [ ] **E2.S2 Query layer.** As the desktop and phone UIs, I want typed queries so that views never write SQL.
  - AC1: Functions for tracks (sorted by any column, filtered by FTS query, returning ordered id lists for view snapshots), albums (grouped by album_artist + album, compilation aware → "Various Artists"), artists, genres, composers, playlist contents 🧪.
  - AC2: Sort uses `sort_*` fields when present and ignores a leading "The " for artist sort (classic jukebox behaviour). Collation is Unicode-aware (ICU via `icu_collator`, registered as a SQLite collation) 🧪.
  - AC3: Building a 50k-track ordered snapshot takes < 60 ms, and fetching a 200-row page by id list takes < 5 ms (bench) 🧪.
  - _Status 2026-11-19: Implemented the typed desktop query layer and V2 sort indexes. Local release medians on Linux were ≤ 19.78 ms for 50k-track snapshots across all 32 sort cases and 0.41 ms for a 200-row page. The benchmark is wired into Linux CI. Cross-platform CI is pending. See ADR-0008._
- [ ] **E2.S3 Device schema.** As the phone, I want a slim schema so that the phone DB stays small and fast.
  - AC1: The device schema from §3.2 is created by its own migration chain 🧪.
  - AC2: The `events` table is append-only and has an `acked` flag. `state` is a key-value table for resume and settings 🧪.
  - _Status 2026-11-19: Device V1, event acknowledgments and JSON state storage implemented. Tests cover independent schema identity, relative media paths, album grouping, immutable event history, and persistence across reopen. Local verification is complete. CI is pending. See ADR-0009._

#### E3 Metadata & scanning (`mt-meta`)
- [ ] **E3.S1 Tag reading.** As a user, I want correct metadata from all my common formats so that my library looks right.
  - AC1: For every fixture format, the extractor returns title, artist, album artist, album, composer, genre, year, track/disc numbers and totals, compilation, sort fields, ReplayGain, duration, codec, sample rate, bit depth, bitrate, channels 🧪.
  - AC2: Missing title falls back to the filename stem. Missing artist or album becomes `NULL` (UI shows "Unknown Artist" / "Unknown Album") 🧪.
  - AC3: Corrupt files return a typed error and never panic (fuzz with `cargo fuzz` for 10 min in CI nightly) 🧪.
  - _Status 2026-11-19: The metadata reader covers all eight fixture formats. Local fixture tests, workspace tests and Clippy pass. A ten-minute AddressSanitizer fuzz run completed 863,830 inputs. Dependency parser panics become typed errors, with an Opus regression test. Fixture checks and nightly fuzzing are wired into CI. Hosted CI is pending. See ADR-0010._
- [ ] **E3.S2 Folder scan & incremental rescan.** As a user, I want to add a folder and have it scanned quickly so that I can start listening.
  - AC1: Parallel scan (rayon) of a folder tree. Supported extensions only. Symlink loops are guarded against 🧪.
  - AC2: Progress events `{scanned, total_estimate, current_path}` at ≥ 4 Hz.
  - AC3: Rescan skips files whose `(size, mtime)` are unchanged. Changed files are re-read. Deleted files are marked `missing=1`, not deleted 🧪.
  - AC4: 10,000 fixture-sized files are scanned in < 60 s on SSD (bench, informational).
  - _Status 2026-11-19: Parallel scanning and SQLite reconciliation are implemented. Local NVMe benchmark imported 10,000 files in 1.22 s and rescanned unchanged files in 35.16 ms, with progress callbacks every 250 ms during file processing. Workspace tests and Clippy pass locally. Hosted CI is pending. See ADR-0011._
- [ ] **E3.S3 Artwork.** As a user, I want album art everywhere so that browsing feels like the real thing.
  - AC1: Embedded art (front cover preferred) is extracted. Otherwise `cover.jpg|folder.jpg|front.*` in the track's folder is used 🧪.
  - AC2: Artwork is deduped by blake3. 300 px and 600 px square JPEG thumbnails (q=85, centre-crop) are generated off the UI thread 🧪.
  - AC3: Tracks of one album share one `artwork_id` 🧪.
  - _Status 2026-11-19: Embedded and folder artwork is integrated into library imports. Worker threads generate both JPEG sizes and SQLite deduplicates by source digest. Local workspace, fixture, and Clippy checks pass. Hosted CI is pending. See ADR-0012._
- [ ] **E3.S4 Tag writing.** As a user, I want edits in Get Info to be saved into my files so that they persist outside MyTunes.
  - AC1: Write title/artist/album/album artist/genre/year/track/disc/compilation/rating/artwork with lofty, preserving unrelated frames 🧪.
  - AC2: Writes are atomic (write temp, then rename) and update `size/mtime` in the DB so the next rescan doesn't treat them as external changes. `file_rev` is **not** bumped 🧪.
  - AC3: Read-only files produce a user-visible error, and the DB is not partially updated 🧪.
  - _Status 2026-11-19: Tag edits stage a verified replacement and update the library fingerprint at the existing file revision. Local tests cover all eight fixture formats, read-only rejection, and restoration after a failed SQLite commit. Hosted CI is pending. See ADR-0013._

#### E4 Audio engine (`mt-audio`)
- [ ] **E4.S1 Decode & output.** As a listener, I want any supported file to play cleanly so that I trust the player.
  - AC1: Plays every fixture format through cpal on Linux, macOS, Windows. Resamples via rubato when needed 🧪 (headless: a null sink that counts frames).
  - AC2: Seek to arbitrary ms is accurate to ±20 ms 🧪.
  - AC3: No allocation or locking in the output callback (reviewed; `assert_no_alloc` in debug builds).
  - _Status 2026-11-19: All 17 fixtures pass decode, seek, and CPAL checks on Linux and native macOS output. The Windows test executable cross-links. Native Windows playback and hosted CI remain pending. See ADR-0014._
- [ ] **E4.S2 Gapless queue.** As a listener of live albums, I want zero gaps between tracks so that albums play as intended.
  - AC1: The gapless-pair fixture produces a continuous waveform: the sample at the boundary equals the expected sine value within 1e-4 🧪.
  - AC2: Encoder delay and padding are honoured for MP3 (LAME header) and AAC (iTunSMPB) 🧪.
  - AC3: If gapless is turned off, a 0 ms boundary with a 5 ms fade-out and fade-in is used instead of a click.
  - _Status 2026-11-19: The streaming queue trims encoder delay and padding, preserves resampler history for matching rates, and supports 5 ms boundary fades. Local fixture and CPAL checks pass for FLAC, MP3, and AAC. Hosted CI is pending. See ADR-0015._
- [ ] **E4.S3 DSP: EQ, volume, limit, Sound Check.** As a listener, I want EQ and volume normalization so that my music sounds the way I like.
  - AC1: 5-band peaking EQ at 60/230/910/3.6k/14k Hz, ±6 dB in 1 dB steps. A frequency response test with a sine sweep is within ±0.5 dB of the target 🧪.
  - AC2: Volume limit caps the output gain at a user-set max 🧪.
  - AC3: Sound Check applies ReplayGain track gain (album gain when playing an album in order). A pre-amp of −6 dB plus the limiter prevents clipping 🧪.
  - AC4: Parameter changes are smoothed over 20 ms, so there are no zipper noises.
  - _Status 2026-11-19: The playback feeder applies EQ, capped volume, track or album Sound Check, and a soft limiter. Local sine sweep, smoothing, fixture metadata, and CPAL checks pass. Hosted CI is pending. See ADR-0016._
- [ ] **E4.S4 Player API & events.** As both UIs, I want one player API so that behaviour is consistent.
  - AC1: API per §3.4. `prev` restarts if elapsed > 4 s, else goes to the previous track (wrapping). `next` wraps. Both reset position to 0 (the handoff's rules) 🧪.
  - AC2: Event stream emits state changes immediately and position at 4 Hz. Track end auto-advances to the next queue item (not loop) 🧪.
  - AC3: Shuffle (songs / albums) and repeat (off / one / all) modes on the queue 🧪.
  - AC4: `mt-cli play <folder>` plays a folder gaplessly with `--eq 3,1,-2,4,2` (manual test, documented).
  - _Status 2026-11-19: The shared player implements transport, events, shuffle and repeat. Linux CLI scan, rescan, and gapless playback with EQ pass against an ALSA null device. Hosted CI is pending. See ADR-0017._
- [ ] **E4.S5 UI sound-effect channel.** As a wheel user, I want click sounds with no latency so that the wheel feels mechanical.
  - AC1: `play_sfx(Click)` mixes a ≤ 15 ms synthesized click into the output within one callback period (< 10 ms at 48 kHz/256 frames).
  - AC2: Works when music is paused (the stream stays open while the app is in the foreground). Respects the system silent switch on iOS (ambient behaviour for sfx).
  - _Status 2026-11-19: The CPAL click starts in the next 256-frame callback and plays over music or silence while paused. The audio crate cross-checks for Android. On a physical iPhone 13 Pro, clicks are audible with Silent mode off and muted with it on, and music continues when switching to Silent mode. iPhone acoustic onset timing and hosted CI remain pending. See ADR-0018._

---

### M2 — Desktop MVP (Tauri 2 + SolidJS 2.0)

> Frontend rules: no domain logic in TS. The frontend renders state and calls typed commands. Sorting, filtering, counting and every file operation happen in Rust (`mt-app`). Components use only `tokens.css` variables for colour, type, spacing and radius.

#### E7 Desktop shell & browsing
- [ ] **E7.S0 Desktop visual mock.** 🎨 design-gate. As the owner, I want to approve the desktop look before it's built so that it feels like a coherent sibling of the iPod app.
  - AC1: A static Solid page (dummy data, no IPC) of the main window in **light (2a palette: white/silver/blue)** and **dark (1a palette: ink/panel/amber)**, runnable via `pnpm dev:mock`. Screenshots are saved to `design/desktop/`.
  - AC2: The layout follows the structure below. Typography is Plex Sans and JetBrains Mono. Uses only `tokens.json` families plus any new desktop tokens, which are listed explicitly and added to `tokens.json`.
  - AC3: Owner sign-off is recorded in `docs/decisions/`. Mock components become the real components (no throwaway).
  - _Status 2026-11-19: The static Solid mock runs via `pnpm dev:mock`. The owner approved both theme screenshots in `design/desktop/`. Hosted CI is pending. See ADR-0019._

  Structural spec for the main window (2005-era jukebox):
  ```
  ┌───────────────────────────────────────────────────────────────────────────┐
  │ ◀◀  ▶  ▶▶   ──●─── vol    ┌──────── LCD status panel ────────┐  [🔍 Search ] │
  │                           │ Title — Artist — Album  0:42 ━━━ │  [Browse]    │
  ├─────────────┬─────────────┴──────────────┬──────────────────┴──────────────┤
  │ LIBRARY     │ Genre        │ Artist       │ Album                          │  ← column browser (toggle)
  │  ♪ Music    ├──────────────┴──────────────┴────────────────────────────────┤
  │ DEVICES     │ ✓ │ Name          │ Time │ Artist │ Album │ Genre │ ♥ │ Plays  │  ← virtualized track table
  │  ▣ Phone ⏏  │   │ ...                                                       │
  │ PLAYLISTS   │                                                               │
  │  ⚙ Smart…   │                                                               │
  │  ≡ Manual…  │                                                               │
  ├─────────────┴───────────────────────────────────────────────────────────────┤
  │ [+] [⇄] [↻] [art]      1,284 songs, 3.4 days, 8.12 GB              [⚙]      │  ← status bar
  └─────────────────────────────────────────────────────────────────────────────┘
  ```
- [ ] **E7.S1 IPC foundation.** As a frontend developer, I want typed, efficient access to the backend so that UI code is simple and fast.
  - AC1: Implements §3.5: tauri-specta bindings generated on build, `Channel`-based subscriptions (player, scan, sync), typed global events, the `view_open`/`view_rows` paging API, and the `mtart` artwork protocol with a single TS `artUrl(id, size)` helper that handles the Windows URL form 🧪 (Rust tests for commands; Vitest with `mockIPC` for adapters).
  - AC2: Capabilities file grants only the commands and plugin permissions actually used. The CSP from §3.5 is set. A test asserts that loading a remote URL is blocked (E2E).
  - AC3: Backend errors are a typed `AppError` enum serialized to TS as a discriminated union. Every command returns `Result<T, AppError>`.
  - _Status 2026-11-19: Typed commands, channels, revision events, paged views, and the artwork protocol are in place. Linux Rust and frontend tests and the real-webview IPC and remote-content smoke test pass. Hosted CI is pending. See ADR-0020._
- [ ] **E7.S2 Main window layout.** As a user, I want the classic layout so that it feels like the jukebox I remember.
  - AC1: Implements the approved mock: transport, volume, LCD status panel, search, source list, optional column browser, track table, status bar. The native window title bar is kept for v1.
  - AC2: Window size and position persist (Tauri `window-state` plugin). Split positions, visible columns and widths, and sort persist via backend settings 🧪.
  - AC3: Theme follows the system light/dark preference (`prefers-color-scheme`), with an override in Preferences that sets `data-theme`.
  - AC4: `single-instance` plugin: launching a second copy focuses the first.
  - _Status 2026-11-19: The approved layout is the native app's main window. Layout and theme settings restore on reload. Linux native tests verify window position and size on reopen and focus on a second launch. Both themes render in WKWebView with fixture IPC. Hosted CI is pending. See ADR-0021._
- [ ] **E7.S3 Import.** As a user, I want to add music by menu, drag and drop, or watched folder so that getting music in is effortless.
  - AC1: File ▸ Add Folder to Library… (Tauri `dialog` plugin) and drag-and-drop of files or folders onto the window (Tauri drag-drop event) both start a scan with a progress indicator in the LCD panel ("Importing 312 of 1,204").
  - AC2: Preferences ▸ Library lets me choose **"Copy files to MyTunes Media folder when adding"** (default off) and add or remove watched folders (notify-based in Rust, debounced 2 s) 🧪.
  - AC3: Duplicate paths are ignored. A duplicate content hash shows a "Duplicate found" note but still imports.
  - _Status 2026-11-19: Native File menu import, mixed file-and-folder drop, LCD progress, duplicate notice, and copy/watch preferences passed Linux verification. The watcher waited two seconds after the last change. Native macOS and Windows verification and hosted CI are pending. See ADR-0022._
- [ ] **E7.S4 Track table.** As a user, I want a fast, sortable, configurable song list so that big libraries stay manageable.
  - AC1: The `VirtualTable` component is built in-house (from the spike), with fixed row height, overscan, and keyboard focus that survives scrolling. Only visible rows plus overscan are in the DOM 🧪 (Vitest: 50k rows render ≤ 80 row elements).
  - AC2: Columns: ✓ (include in sync), Name, Time, Artist, Album, Album Artist, Genre, Year, Track #, Rating (5 stars), Plays, Last Played, Date Added, Kind (codec + bitrate), Size. Right-click the header to toggle columns.
  - AC3: Click a header to sort (asc/desc), handled by the backend snapshot. Album sort groups by album then disc and track.
  - AC4: Multi-select (shift and ctrl/cmd), ⏎ plays, space toggles play/pause, Delete removes from library with a confirm dialog that offers "Also move files to Trash" (default unticked; Rust `trash` crate).
  - AC5: 50k rows scroll at ≥ 55 fps on WebKitGTK (carries over the spike's bar).
  - AC6: The now-playing row shows a speaker glyph. A double-click starts playback from that row with the current view as the queue. Right-click ▸ "Show in File Manager" uses the `opener` plugin's reveal.
  - _Status 2026-11-19: Production WebKitGTK averaged 60.5 and 60.4 fps on 50,000 tracks at 200 and 1,200 px/s. Linux checks passed for selection, focus, column toggles, playback, reveal, and default and opt-in Trash removal. Native macOS and Windows verification and hosted CI are pending. See ADR-0023._
- [ ] **E7.S5 Column browser & search.** As a user, I want to narrow by genre, artist and album, and to search instantly, so that I find music fast.
  - AC1: The browser (View ▸ Show Column Browser) filters progressively: Genre → Artist → Album. "All (n)" rows at the top. Compilations are grouped under "Compilations" in Artist.
  - AC2: The search box filters the table as you type (FTS5 prefix match across fields, 80 ms debounce). Results for 50k tracks arrive in < 50 ms 🧪.
  - AC3: Ctrl/⌘+F focuses search. Esc clears it.
- [ ] **E7.S6 Playback integration.** As a user, I want playback controls and OS media integration so that it behaves like a real player.
  - AC1: Transport buttons, seek in the LCD panel (click and drag), volume slider, all wired to `mt-audio` in the backend.
  - AC2: Media keys and OS integration via souvlaki in the Rust process: MPRIS on Linux (title, artist, art, position), Now Playing on macOS, SMTC on Windows.
  - AC3: A play increments `play_count` and sets `last_played_at` once ≥ 50% of the track or 4 minutes has played, whichever comes first. A skip increments `skip_count` when skipped after 2 s and before 20 s 🧪.
  - AC4: Playback continues when the window is minimized or hidden, because audio doesn't depend on the webview. Quitting saves the queue, track and position. Relaunch restores them paused.
- [ ] **E7.S7 Status bar & stats.** As a user, I want the iconic library summary so that I know the size of my collection.
  - AC1: Shows "N songs, X days/hours, Y GB" for the current view (selection-aware: "3 of 1,284 songs selected" style when > 1 selected). Computed in Rust.
  - AC2: Updates within 100 ms of a filter change.

---

### M3 — Mobile MVP: the wheel

> All mobile stories implement `design/handoff/README.md` **exactly**. When the README and this plan disagree on a visual detail, the README wins. Raise any conflict in an ADR.

#### E5 Mobile foundation
- [ ] **E5.S1 App shell & theming.** As a user, I want the app to look exactly like the design in either theme so that it feels crafted.
  - AC1: One widget tree, with colours resolved through a `MtTheme` built from generated tokens. `dark` = 1a set, `light` = 2a set.
  - AC2: Theme follows system appearance by default. The status bar style switches (light glyphs on dark, `darkContent` on light). No custom status bar is drawn.
  - AC3: Fonts: IBM Plex Sans 400/500/600/700, JetBrains Mono 400/500 bundled. The OFL licence is included in the app's licences page.
  - AC4: Portrait-locked for v1 (landscape is a design gap).
  - AC5: Golden tests for each screen in both themes, at 402×874 logical px, against `design/handoff/screens/`. Tolerance ≤ 1.5% pixel diff, excluding album-art placeholders 🧪.
- [ ] **E5.S2 Rust bridge facade.** As the Flutter app, I want a narrow, async Rust API so that UI code stays simple.
  - AC1: `mt-ffi` exposes: `open_device_db(path)`, `library_counts()`, `albums(page)`, `album_tracks(id)`, `artists()`, `songs(page)`, `playlists()`, `playlist_tracks(id)`, `artwork_path(id, size)`, the player API (§3.4), `player_events() -> Stream`, `settings_get/set`, `record_event(...)`, and the sync client API (M4).
  - AC2: All calls are async and never block the UI isolate. A test calls each from Dart 🧪.
  - AC3: Rust panics are caught at the FFI boundary and surfaced as Dart exceptions with a message.
- [ ] **E5.S3 Dev library seeding.** As a developer, I want to populate the phone without sync so that I can build M3 before M4.
  - AC1: Debug builds only: a hidden "Dev import" action (long-press the header eyebrow for 3 s) opens the OS file picker, copies the chosen files into the app sandbox and builds the device DB using `mt-meta` (the only place the phone reads tags; compiled out of release builds).
  - AC2: Alternatively `tools/seed_device` writes a device DB plus files that can be pushed with `adb push` or Xcode's container download.
- [ ] **E5.S4 Navigation stack.** As a user, I want MENU to go back like an iPod so that navigation is muscle memory.
  - AC1: A screen stack (not go_router routes). Screen change is a horizontal slide: forward slides left, back slides right, 220 ms ease-out.
  - AC2: **Per the handoff, MENU returns to the root Menu and resets selection to 0** for the v1 five-screen set. For the deeper tree in M5, MENU pops **one level**, and the selection of the parent is restored, as on a real iPod. Record the switch in an ADR when E10 lands.
  - AC3: The Android system back gesture equals MENU. On iOS, an edge-swipe back is disabled on wheel screens (the wheel is the navigation).

#### E6 Wheel screens (1a / 2a)
- [ ] **E6.S1 Click wheel widget.** 🧪 As a user, I want a wheel that feels mechanical so that browsing is tactile and fun.
  - AC1: Geometry per the handoff: a 252 pt ring, a 104 pt centre button, the MENU / ◀◀ / ▶▶ / ▶❙❙ labels and hit areas at the exact offsets. No tick marks. The wheel gradient and shadows match each theme.
  - AC2: Angle maths: `atan2` from the ring centre. Delta normalised across ±180°. An accumulator emits one **step per 12°**, possibly several per move event. **No inertia** 🧪 (unit tests: seam crossing clockwise and counter-clockwise, 30°/move yielding 2 steps plus carry, direction reversal mid-drag).
  - AC3: Drags start only on the ring band (between the centre-button edge and the outer radius). Touches on the centre or the four buttons are taps, not drags. Buttons don't fire if a drag of > 6° happened.
  - AC4: Each step fires a selection haptic **with no perceptible latency** (prepared generator. If Flutter's `HapticFeedback.selectionClick` adds > 10 ms on iOS, add a small native plugin using a pre-`prepare()`d `UISelectionFeedbackGenerator`). Each button press fires a light impact.
  - AC5: Visual ring rotation follows the cumulative `wheelRotation`.
  - AC6: The hint text under the wheel is `DRAG RING TO BROWSE / SCRUB / SET GAIN` by context. It's `text-30` normally and accent-coloured while dragging.
  - AC7: The centre label reflects context: `SELECT` (Menu) · `PLAY` (Albums, Queue) · `PAUSE`/`PLAY` (Now Playing) · `BAND <hz>` (Equalizer). The `▶ ❙❙` label is static.
- [ ] **E6.S2 Display header & Menu screen.** As a user, I want the root menu so that I can reach everything.
  - AC1: The header row shows the accent eyebrow on the left and `text-40` metadata on the right (`N songs · M albums`) with the `rule` underline, per the handoff.
  - AC2: Menu rows: Now Playing · Albums (count) · Songs (count) · Queue (count) · Equalizer. 54 pt rows, dot + label + mono count. Selection styling differs by theme (dark: amber-14 tint, amber dot, white label, radius 10. Light: solid blue bar, white text, white dot, radius 5).
  - AC3: Step moves the selection, clamped (no wrap). Centre opens the destination. **Songs opens a full song list** (the prototype routes it to Queue. Treat the real Songs list as a reuse of the Queue row component, without the now-playing highlight).
  - AC4: "Now Playing" is shown but disabled at `text-35` when nothing has ever been played (design gap: owner to confirm).
- [ ] **E6.S3 Albums grid.** As a user, I want to browse album art so that I can pick music visually.
  - AC1: 2-column grid, 14 pt gap, square tiles at radius 6. Real artwork (600 px thumb) or a placeholder colour tile from the theme's placeholder palette indexed by `album_index % 6`, with the uppercase mono caption. Titles and artists below.
  - AC2: The selected tile has a 2 pt accent ring and an accent title. The grid auto-scrolls to keep the selection visible (scroll animation ≤ 120 ms).
  - AC3: Centre plays the album from track 1 (queue = album tracks in disc/track order) and goes to Now Playing.
  - AC4: 500 albums scroll smoothly: artwork decode off the main isolate, cached via `ResizeImage` at 2× tile size.
- [ ] **E6.S4 Now Playing.** As a listener, I want the Now Playing screen so that I can see and control what's playing.
  - AC1: Layout per handoff: 140×140 art (radius 8) + title/artist + format badge (`FLAC · 24/96`, `AAC · 256`, `MP3 · 320`, `ALAC · 16/44`), progress track 3 pt with elapsed / `-remaining` in mono, VOL label + 18 bars (5/7/9 px alternating).
  - AC2: Wheel step = seek ±4 s, clamped. While scrubbing, the audio seeks on release or every 150 ms, whichever comes first (avoid seek storms). Centre toggles play/pause. ◀◀/▶▶ per the §3.4 rules.
  - AC3: Volume is shown as 18 bars reflecting app volume (default 0.62 per handoff). v1 has no wheel mode for volume. **Backlog:** a second centre press switches the wheel to volume mode, as on the real iPod (needs design).
  - AC4: Position updates at 4 Hz from player events. On track end, it advances to the next queue item.
- [ ] **E6.S5 Queue.** As a listener, I want to see what's up next so that I can jump around.
  - AC1: 48 pt rows: zero-padded mono track number, title + artist, duration (`m:ss`). The currently playing row's number and title are accent-coloured, independent of selection (light theme: white when selected, blue when playing and unselected).
  - AC2: On entry, the selection starts on the currently playing row. Centre plays the selected track and goes to Now Playing.
- [ ] **E6.S6 Equalizer.** As a listener, I want to shape the sound with the wheel so that the EQ is fun to use.
  - AC1: 5 bands, a 190 pt band row, centre-anchored bars `height = |gain|/6 × 50%`, Hz labels and signed gains per handoff. The active band is the accent colour, others `accent-42`/`accent-half`.
  - AC2: Wheel step = ±1 dB on the active band, clamped ±6. Centre = next band (wrap). Changes are applied to `mt-audio` live (smoothed).
  - AC3: Status rows: Gapless playback (ON) · Volume limit (OFF) · Shuffle album order (ON) defaults. **Design gap:** the wheel can't reach these rows in the handoff. v1 makes them reachable by stepping past band 5 (selection moves into the rows, and centre toggles). Confirm with the owner 🎨 design-gate.
  - AC4: Default gains `[3, 1, -2, 4, 2]` (handoff). Persisted.
- [ ] **E6.S7 Persistence & resume.** As a user, I want the app to resume where I left it so that it feels like a device, not an app.
  - AC1: Persist `trackIndex`/queue, position, volume, eqGains, settings and theme to `device.sqlite state` on every change (debounced 1 s) and on background 🧪.
  - AC2: Cold start restores the last screen (Menu if > 1 h since last use), queue and position, paused.
- [ ] **E6.S8 Background & lock screen.** As a listener, I want music to keep playing with the screen off so that it's a real player.
  - AC1: iOS: `UIBackgroundModes=audio`, AVAudioSession `.playback`. Android: audio_service foreground service with a media notification.
  - AC2: Lock screen and notification show title, artist, art and progress, with play/pause/next/prev/seek wired to Rust.
  - AC3: Interruptions (calls, Siri, alarms) pause and resume per OS policy. Headphone or Bluetooth disconnect **pauses**. Audio focus loss on Android ducks or pauses.
  - AC4: 1 h of locked playback drains < 6% battery on a reference device (informational; record numbers).
- [ ] **E6.S9 Accessibility baseline.** As a VoiceOver/TalkBack user, I want to reach every item without the wheel so that the app is usable for me.
  - AC1: Each list screen exposes a semantic list. Swipe navigation moves the selection. Double-tap activates. The wheel's buttons are labelled buttons ("Menu", "Previous", "Next", "Play/Pause", "Select") 🧪 (semantics tests).
  - AC2: Selection is announced ("Albums, 6, selected, 2 of 5").
  - AC3: Hit targets ≥ 44 pt. Contrast pairs from the handoff are respected (no meaningful text at ≤ .35 alpha).

**M3 exit:** on a physical iPhone and Android phone, with a dev-seeded library, all five screens in both themes pass golden tests. Background playback works. The owner does a "feel review" of the wheel and signs it off.

---

### M4 — Sync v1

#### E8 Sync protocol & desktop server
- [ ] **E8.S1 Protocol spec.** As both implementers, I want a written spec so that client and server agree.
  - AC1: `docs/sync-protocol.md` fully specifies §3.3: endpoints, JSON schemas, error codes, state machine, merge semantics, versioning, security model.
  - AC2: `mt-sync-proto` contains the types and **pure** functions `plan_sync(desktop_state, device_state, selection) -> SyncPlan` and `apply_events(...)`, with property tests (proptest): the plan never exceeds the free-space budget, never removes a track the selection includes, and is idempotent 🧪.
- [ ] **E8.S2 Desktop sync server.** As a desktop user, I want my computer to offer sync on my LAN only when I allow it so that it's safe.
  - AC1: Preferences ▸ Devices ▸ "Allow Wi-Fi sync" (default **on** after the first pairing, **off** before). The server runs on the Tauri async runtime inside the desktop process, binds to all LAN interfaces on a random high port (or a fixed one if set), and stops cleanly on app exit.
  - AC2: rustls with an rcgen self-signed server cert (generated once and stored in the app data dir). Client certs are required and checked against `devices.cert_fingerprint` for all routes except `/v1/hello` and `/v1/pair` 🧪.
  - AC3: Advertises `_mytunes._tcp` via mdns-sd while enabled. Stops advertising when disabled.
  - AC4: Integration test: in-process server and `mt-sync-client` complete pair → sync → commit using fixtures 🧪.
- [ ] **E8.S3 Pairing UI (desktop).** As a desktop user, I want to pair a phone by scanning a code so that setup takes seconds.
  - AC1: Devices ▸ "Pair a phone…" opens a sheet with a QR code (rendered as SVG in the frontend from a backend-provided payload; `qrcode` crate → SVG string), a 6-word fallback code, and a 5-minute countdown. The token is single-use.
  - AC2: On success the sheet shows "Paired with <device name>", and the device appears in the source list under DEVICES.
  - AC3: Wrong or expired tokens are rejected. More than 5 attempts per minute → 429 🧪.
  - AC4: Device ▸ "Forget this device" revokes the fingerprint. The phone then gets 401 and shows "This phone is no longer paired".

#### E9 Device sync (desktop UI + phone client)
- [ ] **E9.S1 Device page (desktop).** As a desktop user, I want to choose what goes on my phone so that I control the space.
  - AC1: Selecting the device in the source list shows: name, model, capacity, a **capacity bar** (segments: Audio (accent), Other (grey), Free) computed from the last device report, last sync time.
  - AC2: Music tab options: ◉ Entire music library ○ Selected playlists, artists, albums and genres (checkable lists) ○ Manually manage music (M5, E11.S4). ☐ Only sync checked songs (uses the ✓ column). Reserve free space: [1 GB ▾].
  - AC3: As options change, the bar shows the projected usage. If the selection exceeds the budget, the bar turns to an "over capacity" state and Sync is disabled with "Not enough space on <device>: remove X GB" 🧪 (plan_sync).
  - AC4: A "Sync" button plus auto-sync when the phone connects with the app open (preference, default on).
- [ ] **E9.S2 Phone pairing flow.** 🎨 design-gate. As a phone user, I want to pair with my computer easily so that I can get music on.
  - AC1: Empty-library state on first launch: header eyebrow `Library`, a Menu-styled row list with "Pair with computer" as the only selectable item and help text using the Menu row font. (Design gap: needs owner approval.)
  - AC2: "Pair with computer" opens the camera (mobile_scanner) to scan the QR. "Enter code instead" accepts the 6-word code and discovers the desktop via bonsoir.
  - AC3: iOS local-network and camera permission strings are present and human-friendly. Denial shows how to enable it in Settings.
  - AC4: On success, the phone stores the desktop fingerprint and its own key pair in the platform secure store (Keychain / Android Keystore via `flutter_secure_storage`) and starts the first sync.
- [ ] **E9.S3 Sync client & atomic commit.** As a phone user, I want sync to never leave my library broken so that I can trust it.
  - AC1: Implements the §3.3 session in `mt-sync-client`. Files go to `tmp/`, are blake3-verified, then renamed into `music/<2-char-prefix>/<track_id>.<ext>`.
  - AC2: Metadata, playlists and removals are applied in **one transaction** after all files are present. Orphaned files are deleted after commit 🧪.
  - AC3: Killing the app at any point (tested at 10 injected crash points) leaves the previous library fully playable, and the next sync resumes without re-downloading verified files 🧪.
  - AC4: Storage: iOS `Library/Application Support/Music` with `isExcludedFromBackup=true`. Android `getExternalFilesDir(MUSIC)` (app-specific, no permissions).
  - AC5: Sync is allowed only while the app is in the foreground. Backgrounding pauses it. Returning resumes it. The screen is kept awake during sync (wakelock_plus).
- [ ] **E9.S4 Sync screen (phone).** 🎨 design-gate. As a phone user, I want to see sync progress so that I know what's happening.
  - AC1: A full-screen state replaces the display area (the wheel is inert except MENU = "Cancel sync?" confirm): eyebrow `Syncing`, the desktop name, "Do not disconnect." in Plex Sans 600, a progress bar using the Now Playing progress style, "Copying 12 of 340" in mono, and the current track title.
  - AC2: Cancel leaves the previous library intact (rollback semantics of E9.S3).
  - AC3: Completion shows "Sync complete" for 1.5 s, then returns to Menu with updated counts.
- [ ] **E9.S5 Play stats round-trip.** As a user, I want plays on my phone to count on my computer so that smart playlists like "Most Played" stay true.
  - AC1: The phone records play events (same ≥ 50% / 4 min rule as E7.S6), skips, and rating changes into `events` 🧪.
  - AC2: The desktop applies them idempotently. Re-sending the same batch changes nothing 🧪.
  - AC3: After sync, desktop Plays/Last Played reflect phone listening. Acked events are pruned on the phone after 30 days.
- [ ] **E9.S6 Device reporting & eject.** As a desktop user, I want to see when my phone is connected so that I know sync is possible.
  - AC1: The device row shows online/offline (mDNS presence or last request < 60 s) and a ⏏ "Disconnect" that ends any session cleanly.
  - AC2: The phone reports free/capacity bytes at each `begin`. The capacity bar reflects it.

**M4 exit:** from a fresh install, pair, sync 2,000 tracks (≈ 10 GB) over 5 GHz Wi-Fi in < 15 min, play them, sync again with no changes in < 10 s, listen, sync, and see play counts on the desktop.

---

### M5 — Library depth

#### E10 Mobile: full iPod menu tree
- [ ] **E10.S1 Menu tree.** As a user, I want the full classic menu so that I can browse my music every way an iPod could.
  - AC1: Root: **Music ▸**, **Shuffle Songs**, **Now Playing** (when applicable), **Queue**, **Equalizer**, **Settings ▸**. Music: Playlists · Artists · Albums · Songs · Genres · Composers. Artist → Albums (with "All Songs" at top) → Tracks. Genre → Artists → … Lists reuse the Menu row and Queue row components 🎨 design-gate (one review of the generic list screen covers all).
  - AC2: MENU pops one level and restores the parent's selection (ADR per E5.S4 AC2).
  - AC3: Long lists show an **alphabet jump overlay** when the wheel is spun fast (> 6 steps within 250 ms): a large letter centred on the display. The selection jumps by first-letter groups until the spin slows. 🎨 design-gate.
  - AC4: "Shuffle Songs" shuffles the whole library and goes to Now Playing.
- [ ] **E10.S2 Shuffle & repeat.** As a listener, I want shuffle and repeat settings so that I can listen my way.
  - AC1: Settings ▸ Shuffle: Off / Songs / Albums. Settings ▸ Repeat: Off / One / All. These map to E4.S4 modes 🧪.
  - AC2: The Now Playing header metadata shows a small mono indicator (`SHUF` / `RPT1` / `RPT`) when active 🎨 design-gate.
- [ ] **E10.S3 On-The-Go playlist.** As a listener, I want to build a playlist on the phone so that I can queue up music on the go.
  - AC1: On any track or album row, a **centre long-press (≥ 600 ms)** adds it to On-The-Go, with a visual flash on the row and a haptic.
  - AC2: Music ▸ Playlists ▸ On-The-Go lists it. It can be cleared ("Clear Playlist" row → confirm).
  - AC3: On sync it is uploaded as an OTG event, becomes desktop playlist "On-The-Go 1, 2, …", and the phone's OTG is cleared after ack 🧪.
- [ ] **E10.S4 Settings.** As a user, I want device settings so that I can tailor the player.
  - AC1: Settings rows (status-row style): Theme (System / Dark / Light), Shuffle, Repeat, Sound Check (On/Off), EQ preset (list), Volume limit, Clicker (Sound / Haptic / Both / Off), Gapless, About (version, library counts, desktop name, last sync, licence and source link), Unpair.
  - AC2: All persist and apply immediately 🧪.
- [ ] **E10.S5 Ratings on device.** As a listener, I want to rate the playing song so that my ratings sync back.
  - AC1: On Now Playing, a centre press cycles a display mode: (progress) → (rating). In rating mode a wheel step sets 0–5 stars and the next centre press returns 🎨 design-gate.
  - AC2: Rating changes produce events that sync back (E9.S5).

#### E11 Desktop: playlists, editing, organization
- [ ] **E11.S1 Manual playlists & folders.** As a user, I want playlists so that I can curate music.
  - AC1: Create, rename, delete. Drag tracks from the table onto a playlist in the source list (HTML5 drag-and-drop within the webview; payload = track ids). Reorder within the playlist. Playlist folders, nested 🧪.
  - AC2: The same track can appear multiple times. Removing from a playlist never deletes from the library.
- [ ] **E11.S2 Smart playlists.** As a user, I want rule-based playlists so that they keep themselves up to date.
  - AC1: The editor supports rules on any column, with ops (is, contains, starts with, >, <, in range, in the last N days) and match **all/any**, plus nested rule groups. Limit to N items/minutes/GB selected by random / most played / least recently played / highest rated. "Live updating" toggle.
  - AC2: `mt-smart` compiles rules to parameterized SQL, never string interpolation 🧪. The rule model is a specta type shared with the TS editor.
  - AC3: Built-ins created on first launch: "Recently Added" (last 30 days), "Most Played" (top 25), "Recently Played", "Top Rated" (≥ 4 stars).
- [ ] **E11.S3 Get Info.** As a user, I want to edit song details so that my library is tidy.
  - AC1: Ctrl/⌘+I opens a sheet with Summary (codec, size, bitrate, sample rate, path, date added, plays) / Info (editable tags) / Artwork (add, replace, remove) / Options (rating, Sound Check gain readout, "Remember position", "Skip when shuffling").
  - AC2: Multi-select edit shows "Mixed" for differing fields and only writes changed fields.
  - AC3: Saving writes to files (E3.S4) and the DB. Synced devices get the updated metadata at next sync without a file re-send 🧪 (assert no `/files` request).
- [ ] **E11.S4 Manually manage music.** As a power user, I want to drag songs onto my phone so that I control exactly what's there.
  - AC1: Device option "Manually manage music": the device page lists its tracks, and dragging tracks or playlists onto the device row queues them. Delete removes them from the device.
  - AC2: Switching back to automatic warns that manual changes will be replaced.
- [ ] **E11.S5 Organize library.** As a user, I want MyTunes to keep my files tidy so that my folder structure is clean.
  - AC1: Preference "Keep MyTunes Media folder organized": moves files into `<Album Artist>/<Album>/<disc-track> <Title>.<ext>`, sanitized for all OSes, after tag edits and on import-with-copy 🧪.
  - AC2: Moves update `file_path` atomically and never bump `file_rev`.
- [ ] **E11.S6 Missing-file handling.** As a user, I want to recover files I've moved so that my library stays intact.
  - AC1: Tracks with `missing=1` show a "!" in the table. Play attempts show "The song could not be used because the original file could not be found. Locate?" Locate updates the path and offers to search the same folder for other missing files.
- [ ] **E11.S7 Artwork polish.** As a user, I want to set album art in bulk so that albums look complete.
  - AC1: Drag an image onto the artwork well (status bar art toggle) for selected tracks or an album. Art is stored per E3.S3, and devices receive the new thumbs at next sync.
  - AC2: The album view (View ▸ as Albums) shows a virtualized grid of covers, matching the iPod Albums grid feel.

---

### M6 — Retro polish & accessibility

#### E12 Feel & delight
- [ ] **E12.S1 Clicker.** As a user, I want an audible click on each wheel detent so that it feels like the real thing.
  - AC1: Clicker setting (Sound / Haptic / Both / Off, default Both). The sound plays via the E4.S5 sfx channel. iOS respects the silent switch for the clicker.
  - AC2: The click is our own sound asset (synthesized or recorded by us; licence noted in `assets/LICENSES.md`).
- [ ] **E12.S2 Wheel acceleration (opt-in).** As a user with a huge library, I want faster scrolling so that I don't spin forever.
  - AC1: An optional setting where sustained fast rotation (> 3 steps per 100 ms for > 500 ms) emits 2×, then 4× steps. Haptics fire per **emitted** step (handoff note). Default off, preserving the 1:1 handoff behaviour.
- [ ] **E12.S3 EQ presets.** As a listener, I want named EQ presets so that I can switch sounds quickly.
  - AC1: Presets: Flat, Acoustic, Bass Booster, Bass Reducer, Classical, Dance, Electronic, Hip-Hop, Jazz, Loudness, Rock, Spoken Word, Treble Booster, Treble Reducer, Vocal Booster, plus Custom (the EQ screen's live gains). The Equalizer header right-hand metadata shows the preset name (`Custom` per handoff).
  - AC2: The same presets exist on the desktop (Window ▸ Equalizer), but desktop and phone EQ are independent settings.
- [ ] **E12.S4 Desktop LCD panel polish.** As a user, I want the status panel to feel alive so that the app has character.
  - AC1: Click the LCD to cycle the subtitle line: artist → album → "Elapsed/Remaining" toggle. During import or sync it shows progress. During device sync it shows "Syncing '<device>' (Copying 12 of 340)".
- [ ] **E12.S5 Sound Check.** As a listener, I want consistent loudness so that I don't keep reaching for volume.
  - AC1: The desktop computes ReplayGain (EBU R128 via the `ebur128` crate) for tracks without tags. Run as a background task (Advanced ▸ "Analyze loudness"). Values sync to devices as metadata 🧪.

#### E13 Empty, error & edge states (🎨 design-gate for each new state)
- [ ] **E13.S1 Mobile empty & error states.** AC1: Empty library (E9.S2). Track file missing on device (skip with a toast-style header message and flag for re-sync). Desktop unreachable during sync ("Can't find <desktop>. Make sure MyTunes is open on the same Wi-Fi."). Protocol mismatch. Storage full mid-sync (abort cleanly, report bytes needed).
- [ ] **E13.S2 Missing artwork.** AC1: Placeholder tiles from the theme palette with the album initials or a short title caption (handoff placeholder style) whenever art is absent, on both phone and desktop. The caption disappears when real art is present.
- [ ] **E13.S3 Desktop errors.** AC1: Unreadable files, failed tag writes, DB locked, port in use, webview crash. Each has a clear message, and none crash the backend. A log file in the app data dir (Tauri `log` plugin + `tracing`) records them. Help ▸ "Show Logs" opens the folder.

#### E14 Accessibility & scale
- [ ] **E14.S1 Dynamic Type / font scale.** 🎨 design-gate. AC1: Mobile text scales up to 135% without overlap, reflowing rows to two lines where needed. The wheel stays fixed-size. Desktop respects the OS and webview zoom (Ctrl +/−) without layout breakage at 150%.
- [ ] **E14.S2 Screen readers (full pass).**
  - AC1: Mobile: complete VoiceOver/TalkBack audit of every screen.
  - AC2: Desktop: the track table uses ARIA grid semantics (`role="grid"`, `aria-rowcount`, `aria-rowindex` on virtualized rows), the source list is a `tree`, and dialogs trap focus. Tested with Orca (Linux), VoiceOver (macOS) and NVDA (Windows).
  - AC3: Desktop: keyboard-only operation of all features (Tab order, shortcuts listed in Help ▸ Keyboard Shortcuts).
- [ ] **E14.S3 Big-library performance.** AC1: Phone with 20k tracks: cold start < 1.5 s to an interactive Menu, wheel step → UI update < 16 ms (profile mode), Albums grid at ≥ 58 fps. Desktop with 100k tracks: launch to an interactive table in < 2 s, search < 50 ms, idle CPU < 1% with paused playback.

---

### M7 — Release (GitHub only)

#### E15 Packaging & distribution
- [ ] **E15.S1 Desktop packages.**
  - AC1: `tauri-apps/tauri-action` on version tags (`v*`) builds and attaches to a draft GitHub Release: Linux AppImage, `.deb`, `.rpm`. macOS universal `.dmg`. Windows NSIS `.exe` and `.msi`.
  - AC2: Signing:
    - macOS: notarized if the owner adds Developer ID secrets. Otherwise unsigned, with README steps for Gatekeeper ("Open Anyway" / `xattr -dr com.apple.quarantine`).
    - Windows: unsigned unless a certificate is provided. The README notes the SmartScreen prompt.
  - AC3: Tauri `updater` plugin, signed with a project minisign key stored as a GitHub secret. It reads `latest.json` from GitHub Releases. Update checks are **opt-in** (first-run prompt), with no other network calls.
  - AC4: `packaging/aur/PKGBUILD` for an `mytunes-bin` AUR package from the release AppImage or deb, plus a CI check that it builds in an Arch container. Flatpak/Flathub is backlog.
- [ ] **E15.S2 Mobile builds.**
  - AC1: Android: a release APK (split per ABI plus a universal APK), signed with the project keystore stored as GitHub secrets, attached to the Release. Compatible with Obtainium (stable asset names). Versioning is shared with the desktop tag.
  - AC2: iOS: CI attaches an **unsigned IPA** for re-signing with sideloading tools (AltStore/SideStore). `docs/ios-install.md` documents both paths: (a) build from source in Xcode with a free Apple ID (app expires after 7 days; re-run to refresh) or a paid account (1 year), and (b) sideload the IPA. Required capabilities (background audio, local network) work under free provisioning (verified in E1.S3).
  - AC3: App icons and launch screens 🎨 design-gate.
- [ ] **E15.S3 Licensing & trademarks.**
  - AC1: App names, icons, store-like descriptions and the repo contain no "iPod", "iTunes", "Apple", the Apple logo, or Chicago or Myriad fonts. README wording is descriptive only ("inspired by classic click-wheel players").
  - AC2: Third-party licence pages in both apps: `cargo-about` output for Rust, `LicenseRegistry` for Flutter, and a generated npm licence list for the desktop frontend. The release CI fails if a dependency's licence isn't allowed by `cargo-deny` or the npm check.
- [ ] **E15.S4 Docs.** AC1: The user guide covers install for each platform, pairing, sync options, and troubleshooting (firewall, local network permission, different subnets/VLANs, the NVIDIA/Wayland webview flag). AC2: The developer guide covers the architecture (§3), how to add a format, how to run spikes and fixtures, and the release process (tagging, secrets).

---

## 6. Cross-cutting requirements

| Area | Requirement |
|---|---|
| **Privacy** | No network calls except LAN sync and (opt-in) update checks against GitHub. No analytics or telemetry. No remote fonts, scripts or images in the webview. Crash logs stay local. A user can export logs to attach to GitHub issues. |
| **Security** | Mutual TLS with pinned certs. The pairing token is single-use with a 5-min TTL and rate-limited. The server is off until the user enables it or pairs. Phone keys live in the OS secure store. Desktop keys go in the OS keychain via the `keyring` crate, falling back to an app-data file with 0600 permissions. Path traversal is impossible: the server serves only DB-known track IDs, never paths. Tauri capabilities are least-privilege, with strict CSP (§3.5). Security reports go through `SECURITY.md`. |
| **Reliability** | SQLite WAL, every multi-row change in a transaction. Desktop library backup: keep the last 5 daily copies of `library.sqlite`. |
| **Performance** | Targets listed in E1.S1, E7.S4, E7.S5, E14.S3, M4 exit. |
| **Formats** | MP3, AAC/M4A, ALAC, FLAC (≤ 24/192), WAV, AIFF, Ogg Vorbis. Opus is optional (backlog unless libopus is trivial). DRM-protected files (`.m4p`) are detected and skipped with a message. |
| **i18n** | All user-facing strings in message catalogs from day one: ARB (Flutter) and a small typed JSON catalog in the desktop frontend. English only for v1. Unicode-safe sorting (ICU collation in Rust). The same collation key is sent to the phone as `sort_key` so both sort identically. |
| **Logging** | `tracing` in Rust (filtered by `RUST_LOG`), the Tauri `log` plugin bridging frontend logs, and forwarding to Dart's log in debug builds. |

---

## 7. Risks & open questions

| ID | Risk / question | Impact | Mitigation / decision owner |
|---|---|---|---|
| R1 | **Trademark.** No app-store review applies (GitHub-only distribution, owner decision), but Apple trademarks still matter in a public project. | Takedown request | No Apple names, logos or fonts in app names, icons or repo metadata. Original amber dark palette as default. Descriptive wording only. |
| R2 | **Solid 2.0 is days old.** Point releases will follow, and libraries lag. | Churn, missing libs | Exact pins, peer-dep check before adding libraries, in-house virtual table, ADR-0005 idioms, fallback to 1.9.x (E1.S1). |
| R3 | **WebKitGTK performance and quirks** on the primary platform | Janky table, blank windows | Spike S1 measured on WebKitGTK. DMABUF workaround. Canvas table-body fallback. |
| R4 | Rust audio lifecycle on mobile | Background playback bugs | Spike S2. Fallback is just_audio. |
| R5 | iOS builds need macOS. Free provisioning expires every 7 days. | Slow iteration, user friction | macOS CI from M0. A borrowed or cloud Mac for device testing. Clear sideload docs (E15.S2). |
| R6 | Wi-Fi sync blocked by client isolation, VLANs or firewalls | Users can't sync | QR carries IP candidates. Manual "Connect to IP" entry. Troubleshooting doc. **Backlog:** USB sync. |
| R7 | Lossless libraries exceed phone space | Poor experience | Capacity bar and reserve. **Backlog:** transcode-on-sync (E-B2). |
| R8 | Webview engine differences (WebKit vs Chromium) | CSS bugs on one OS | E2E on Linux and Windows. Manual macOS check per release. Conservative CSS. |
| Q1 | Default theme: follow system (handoff) or always light 2a? | — | **Default: follow system.** Owner to confirm. |
| Q2 | Is "Songs" a full song list (plan) or Queue (prototype)? | — | Plan assumes a full list. |
| Q3 | How should the wheel reach the EQ status rows? | — | E6.S6 AC3 proposal. Owner to confirm. |
| Q4 | Desktop platforms priority order? | — | Assumed Linux → macOS → Windows. |
| Q5 | ~~Project licence?~~ **Decided: `MIT OR Apache-2.0`** (owner, 2026-11-18). | — | Record in ADR-0006. Dependencies must be permissive or weak-copyleft (MPL-2.0 is fine, e.g. symphonia). Strong copyleft (GPL, AGPL) is not allowed. |

---

## 8. Backlog (post-v1, unordered)
- **E-B1 USB sync.** Android via adb/MTP. iOS via libimobiledevice `house_arrest` into the app's Documents (needs `UIFileSharingEnabled`). Gives the "plug in the cable" experience.
- **E-B2 Transcode on sync.** "Convert higher bit-rate songs to AAC 256 / Opus 160" per device (libopus, which is BSD, is preferred; ffmpeg only as a user-installed external binary that is never bundled or linked, to keep the licence permissive). Cache the transcodes.
- **E-B3 Direction 1c hybrid scrub arc** as an alternate Now Playing (absolute angle → position; spec in handoff).
- **E-B4 Direction 1b modern tabs** as an accessibility-friendly alternative UI mode.
- **E-B5 Wheel volume mode** on Now Playing (centre-press toggles; needs design).
- **E-B6 Album flip view** (3D album carousel on desktop and in phone landscape), with an original name and look.
- **E-B7 Import an iTunes / Music `Library.xml`** (playlists, play counts, ratings, date added).
- **E-B8 Last.fm / ListenBrainz scrobbling** from sync events (opt-in).
- **E-B9 Podcasts and audiobooks** (remember position, playback speed).
- **E-B10 Extras menu:** Clock, Stopwatch, Notes (plain text synced from desktop), and an **original** mini-game designed for the wheel.
- **E-B11 CarPlay and Android Auto.**
- **E-B12 Multiple libraries per phone / multiple phones per library** (the data model already allows many devices per library).
- **E-B13 Opus and WavPack support.**
- **E-B14 Flathub and F-Droid.** F-Droid needs a fully FOSS build. `mobile_scanner` uses Google ML Kit on Android, so it would have to be swapped for a ZXing-based scanner in an F-Droid flavour.
- **E-B15 Custom window chrome** on desktop (brushed-metal style title bar via `decorations: false` plus a drag region).

---

## 9. Definition of Done (every story)
1. All ACs pass. Every AC marked 🧪 has an automated test that runs in CI.
2. `cargo fmt`, `cargo clippy -D warnings`, `pnpm typecheck`, `pnpm lint`, `pnpm test`, `flutter analyze`, and all tests pass on CI for affected targets. Generated files (`bindings.ts`, `tokens.css`, `tokens.g.dart`, frb bindings) are up to date.
3. Mobile UI stories: golden tests updated, and a visual check against `design/handoff/screens/` on a physical device or simulator in **both themes**. Desktop UI stories: checked in both themes on WebKitGTK, plus WebView2 or WKWebView when CSS changed significantly.
4. No new warnings, `unwrap()` in non-test code paths that handle user data, or TODOs without a linked backlog item or GitHub issue.
5. Public Rust APIs have doc comments. Changes to sync wire types update `docs/sync-protocol.md` and bump `protocol_v` if breaking.
6. 🎨 design-gate stories have owner approval recorded (link or screenshot in the PR).
7. The checkbox in this file is ticked, with any deviation explained in an ADR.

## 10. Working agreement for the implementing agent
- **Order:** follow milestones in order. Within a milestone, follow story order unless a story says otherwise. M2 and M3 may interleave after M1.
- **Small PRs:** one story per PR. The commit message starts with the story ID, for example `E6.S1: click wheel detent maths`. This is a public repo, so write commit messages and PR descriptions for outside readers.
- **ADRs:** `docs/decisions/NNNN-title.md` (context → decision → consequences) for every spike result, every deviation from this plan, and every owner decision on a Q-item.
- **Solid 2.0:** write 2.0 code only. Check the official 2.0 docs when unsure, never 1.x examples. Before adding any npm dependency, confirm its peer deps accept `solid-js@2` and note it in the PR.
- **Design fidelity:** the handoff README is the spec. Don't eyeball it; use the token values. Never add a visual element the handoff doesn't have without a 🎨 gate.
- **Never** commit copyrighted audio, Apple assets or Apple fonts, the design-tool runtime files (`support.js`, `ios-frame.jsx`), or secrets. Never add analytics.
- **When blocked** (spike no-go, ambiguous AC, missing design): stop, write the question into §7 as a new Q-item, and ask the owner rather than guessing.
