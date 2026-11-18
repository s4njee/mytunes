# 0005: Solid 2.0 idioms for this codebase

- **Status:** Accepted (2026-11-18)
- **Story:** E1.S1 AC5 (mandated by plan §2.3 R2 and §10)
- **Input:** official Solid 2.0 docs and the
  [1.x → 2.0 migration guide](https://docs.solidjs.com/migration/from-solid-1),
  validated by the E1.S1 spike on WebKitGTK.

## Context

Solid 2.0 shipped 2026-11-17. Most online examples and most model training
data are Solid 1.x, which actively misleads: 2.0 changed the execution
model (staged writes, split effects), moved packages (`@solidjs/web` now
owns DOM rendering and JSX types), and removed many 1.x APIs outright
(`onMount`, `batch`, `Index`, `classList`, `on:`/`attr:`/`use:`,
`createResource`, `Suspense`, `ErrorBoundary`). The plan therefore requires
every implementing agent to work from the 2.0 docs and record the idioms
here (plan §10, "Solid 2.0").

## Decision

The rules below are binding for all desktop frontend code. When a future
agent is unsure, this list plus the official docs are the source of truth
— never a 1.x example.

### Packages and JSX

- `solid-js` exports reactivity **and stores** (`createStore` lives in
  `solid-js`; `solid-js/store` is gone). `@solidjs/web` owns `render`,
  `hydrate`, DOM `JSX` types, and the JSX runtime.
- `tsconfig.json` uses `"jsx": "preserve"` and
  `"jsxImportSource": "@solidjs/web"`.
- The Vite plugin is `@solidjs/vite-plugin` (3.x), not `vite-plugin-solid`.

### Execution model

- **Staged writes.** Setters do not take effect synchronously: a read
  right after a setter returns the last *committed* value; commits happen
  on the next microtask. Consecutive writes batch automatically — `batch`
  is removed, never reintroduce it.
- **`flush()`** is for imperative/test boundaries only: tests must
  `flush()` (or await a `findBy*`) after firing events before asserting
  signal state or DOM. Event handlers in production code need no flush.
- **Dev mode rejects writes in owned scopes** (component bodies, memo
  computes). Write from event handlers, actions, or promise continuations
  instead. A fire-and-forget `void (async () => { … })()` at component
  creation is the accepted boot pattern (its writes happen in promise
  callbacks, outside the owned scope).
- Component bodies must not read reactive values at top level outside
  JSX/computations; dev mode warns.

### Primitives

- `createEffect` is **two-arg**: `createEffect(compute, effect)`. The
  compute phase tracks reads and must not write; the effect phase is
  untracked, does imperative work, and returns its own cleanup. No
  single-arg form, no `on(...)`, no `initialValue` argument.
- `onMount` → `onSettled`. `onError`/`catchError` → `<Errored>`
  boundaries or the effect `error` callback. `createComputed` →
  `createMemo` or split `createEffect`.
- **Ref callbacks are not owned scopes**: `onCleanup` inside a ref
  callback throws `[CLEANUP_IN_FORBIDDEN_SCOPE]`. Keep a mutable
  `let observer` and register one body-level `onCleanup(() =>
  observer?.disconnect())`. jsdom will not catch this (no
  `ResizeObserver`), so always smoke a changed component in a real
  engine or the dev webview.
- `createSignal(fn)` (function form) is the writable derived signal; use
  it before inventing write-back effects.

### Stores

- Setters take a **mutable draft**: `setState((draft) => {
  draft.user.city = "Paris"; })`. No path arguments, no `produce` wrapper.
- `reconcile` runs against the selected draft inside the setter.
- `unwrap` → `snapshot`; `mergeProps` → `merge`; `splitProps` → `omit`.

### JSX

- Events are camelCase only (`onClick`); lowercase `onclick` silently
  compiles to an attribute and dev-warns `[LOWERCASE_EVENT_ATTRIBUTE]`.
- `class` accepts object/array forms; `classList` is a compile error.
- `use:`, `on:`, `oncapture:`, `attr:`, `bool:` are compile errors naming
  their replacement.
- `<For>` is keyed by value (the callback receives the item itself).
  `<Index>` is gone — use `<For keyed={false}>` when identity should be
  positional.
- `Suspense` → `<Loading fallback={…}>`; async memos replace
  `createResource` (the spike did not need these — E7 will).

### Testing

- `@solidjs/testing-library` 1.0.0-rc.x works with Solid 2, but its
  auto-cleanup hooks into a global `afterEach`. We run Vitest with
  `globals: false`, so **every test file must call `cleanup()` in its own
  `afterEach`** or roots leak across tests.
- The Vite plugin no longer sets `test.environment`; our
  `vite.config.ts` sets `environment: "jsdom"` explicitly.
- A DOM-less environment (jsdom) cannot run `requestAnimationFrame`
  paths: guard with `typeof requestAnimationFrame === "function"` (see
  `nextPaint()` in the spike) and keep such code out of unit-tested
  paths.

### tauri-specta specifics learned in the spike

- Commands returning `Result<T, AppError>` serialize through a
  `typedError` wrapper that adds `{status:"ok"|"error"}` itself. In
  `mockIPC` tests, return the **bare payload** — a hand-wrapped union gets
  double-wrapped and silently yields `undefined` fields. (This cost the
  spike an hour to find.)
- Tauri command arguments are matched camelCase (`view_id` ⇄ `viewId`),
  so mocks read camelCase keys.
- `tauri::ipc::Channel<T>` works as a specta-typed command argument and
  generates `Channel<PositionEvent>` in `bindings.ts`.
- Commands defined in a submodule must be brought into the registering
  module with a plain `use` **and** `#[macro_use] mod …` for the hidden
  `__cmd__*` macros to be visible to `collect_commands!`.
- specta forbids `i64/u64/usize` in exported types (BigInt precision
  loss): spike wire types use `i32`.

### Lint

- Biome's `a11y/useSemanticElements` and `useFocusableInteractive`
  conflict with a pooled div-grid (see `biome.json` override for
  `VirtualTable.tsx`). This is a documented deferral, not a permanent
  exemption: E7.S4 adds roving tabindex and E14.S2 does the full
  ARIA-grid pass, at which point the override must shrink or disappear.

## Consequences

- Code reviews have a concrete, project-local 2.0 checklist instead of
  1.x folklore.
- Two known foot-guns are pre-solved for later stories (mock double-wrap,
  ref-callback cleanup).
- The VirtualTable a11y override is debt with named owners (E7.S4,
  E14.S2); CI will keep the rest of the a11y rules enforcing.
