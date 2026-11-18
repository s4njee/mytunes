# MyTunes

A retro music player in two parts, inspired by classic click-wheel players and the desktop jukebox apps they synced with:

- **MyTunes Pod** (iOS / Android): a click-wheel player for the music on your phone. Drag around the ring to move through lists, press the centre to choose.
- **MyTunes** (Linux / macOS / Windows): manages your music library and syncs a chosen subset to paired phones over Wi-Fi.

It works with local files only: no streaming, no accounts, no cloud, no telemetry.

> **Status:** early development (milestone M0). Nothing is usable yet. See [`plan.md`](plan.md) for the roadmap.

## Repository layout

| Path | What |
|---|---|
| `crates/` | Shared Rust core: database, metadata, audio engine, sync protocol |
| `apps/desktop/` | Desktop app: Tauri 2 (`src-tauri/`) + SolidJS 2.0 (`src/`) |
| `apps/mobile/` | Mobile app: Flutter, calling the Rust core via flutter_rust_bridge |
| `design/` | Design tokens and the mobile design handoff |
| `tools/` | Code generators (tokens, test fixtures) |
| `docs/decisions/` | Architecture decision records |

## Building from source

### Common prerequisites

- **Rust**: the version is pinned in [`rust-toolchain.toml`](rust-toolchain.toml), and `rustup` installs it automatically.
- **Node** (version in [`.nvmrc`](.nvmrc)) and **pnpm** (version in `apps/desktop/package.json` → `packageManager`).
- **Flutter** (version in `apps/mobile/pubspec.yaml` → `environment.flutter`), for the mobile app only.

### Linux (Arch / CachyOS)

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool alsa-lib
```

For Debian/Ubuntu, install `libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev`.

**NVIDIA + Wayland:** if the desktop window is blank or flickers, run with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

### Desktop app

```bash
cd apps/desktop
pnpm install
pnpm tauri dev
```

End-to-end smoke test against the real webview (Linux and Windows; needs `cargo install tauri-driver --locked`, and `WebKitWebDriver` on Linux, which Arch ships with `webkit2gtk-4.1`):

```bash
pnpm tauri build --debug --no-bundle && pnpm e2e
```

### Rust core

```bash
cargo test --workspace
```

### Mobile app

Android needs the Android SDK and NDK (`ANDROID_HOME`; the NDK version comes from the Flutter project). The Rust library is built by cargokit with the **`stable`** rustup toolchain, so add the Android targets to it:

```bash
rustup target add --toolchain stable aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
```

```bash
cd apps/mobile
flutter pub get
flutter run
```

Run the Rust-backed integration test on a device or emulator with `flutter test integration_test`.

After changing anything in `crates/mt-ffi/src/api/`, regenerate the Dart bindings (needs `cargo install flutter_rust_bridge_codegen --version 2.13.0 --locked`):

```bash
cd apps/mobile && flutter_rust_bridge_codegen generate && cargo fmt --all
```

**Arch: rustup plus the `rust` package.** If both are installed, Gradle may pick `/usr/bin/rustc`, which has no Android targets (error: `can't find crate for core`). Remove the `rust` package, or build with `RUSTC=~/.cargo/bin/rustc`.

**AUR `flutter-bin`:** if `flutter` prints `unionfs failed`, either add yourself to the `flutter` group or put `/opt/flutter/bin` first on your `PATH`.

iOS builds need macOS with Xcode. A free Apple ID is enough to run on your own device; the app then expires after 7 days. See `docs/ios-install.md` (coming in M7).

### Regenerating generated files

```bash
cargo run -p gen-tokens      # design/tokens.json → Dart + CSS
cargo run -p gen-fixtures    # test audio fixtures (needs ffmpeg for lossy formats)
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please report security issues privately; see [SECURITY.md](SECURITY.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

Fonts bundled in the apps (IBM Plex Sans, JetBrains Mono) are under the SIL Open Font License 1.1.

MyTunes is an independent project. It is not affiliated with, or endorsed by, Apple Inc.
