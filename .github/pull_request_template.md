## What

<!-- Story ID(s) from plan.md, e.g. E6.S1, and a short summary. -->

## Checklist (Definition of Done, plan.md §9)

- [ ] All acceptance criteria pass; every 🧪 criterion has an automated test
- [ ] `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `pnpm typecheck/lint/test`, `flutter analyze/test` pass for affected parts
- [ ] Generated files are up to date (`bindings.ts`, `tokens.*`, flutter_rust_bridge bindings)
- [ ] UI changes checked in both themes (screenshots below)
- [ ] 🎨 design-gate approval linked, if the story has one
- [ ] Deviations from plan.md recorded in an ADR; story checkbox ticked in plan.md
- [ ] New npm dependencies accept `solid-js@2` in their peer deps; new crates pass `cargo deny`

## Screenshots
