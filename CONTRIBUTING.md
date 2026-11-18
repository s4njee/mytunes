# Contributing to MyTunes

Thanks for your interest! This project is built in the open from a written plan.

## How work is organised

- [`plan.md`](plan.md) holds the milestones, epics and user stories. Every story has an ID (for example `E6.S1`) and acceptance criteria.
- Decisions that change or refine the plan are recorded as ADRs in [`docs/decisions/`](docs/decisions/) (context → decision → consequences).
- One story per pull request where possible.

## Commit messages

Start the subject with the story ID when there is one:

```
E6.S1: click wheel detent maths
```

Otherwise use a short imperative subject (`Fix seek clamping on short tracks`).

## Before opening a PR

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/desktop && pnpm typecheck && pnpm lint && pnpm test
cd apps/mobile && flutter analyze && flutter test
```

Generated files must be up to date: `bindings.ts` (tauri-specta), `tokens.css` / `tokens.g.dart` (`cargo run -p gen-tokens`) and the flutter_rust_bridge bindings.

## Rules

- **Never commit copyrighted audio.** Tests use generated fixtures (`cargo run -p gen-fixtures`).
- **Never commit Apple assets** (logos, fonts such as Chicago or Myriad, sounds), and don't use Apple trademarks in app names, icons or copy.
- **Don't commit the design tool's runtime files** (`design/handoff/support.js`, `design/handoff/ios-frame.jsx`). They're gitignored.
- **Desktop frontend uses SolidJS 2.0.** Write 2.0 code from the official 2.0 docs, not 1.x examples. Before adding an npm dependency, check that its `peerDependencies` accept `solid-js@2`.
- **Licences:** new dependencies must be permissive (MIT, Apache-2.0, BSD, ISC, Zlib) or MPL-2.0. `cargo deny check` enforces this in CI.
- **No telemetry or analytics,** and no network calls beyond LAN sync and the opt-in update check.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).

## Licence of contributions

By contributing, you agree that your contributions are dual-licensed under MIT OR Apache-2.0, as described in the [README](README.md#license).
