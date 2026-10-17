# API Design — jilva-rs

> **Status:** Phase 2 terminal seam in progress (2026-10-17).
> Principles and facade contract. Normative
> for structure and conventions; illustrative sketches below are explicitly
> **not final and not compiling** — exact signatures are settled by
> RFC-1 (scope), RFC-2 (component model), and RFC-3 (theming).
>
> Related: `docs/ARCHITECTURE.md` (§3 boundaries, §6–§7 contracts),
> `docs/adr/`, `docs/RFC/` (process: Draft → Discussion → Accepted).

## 1. Design principles

1. **Intent over mechanics.** Users describe *what* (`row`, `accent`,
   `on_select`) not *how* (coordinates, ANSI codes, escape sequences).
   Anything requiring manual cell math from app code is an API bug.
2. **Calm defaults, open hood.** The zero-configuration path looks right;
   every default is overridable through a discoverable, documented knob.
3. **Progressive disclosure.** Three layers: (a) build-and-run helpers for
   simple apps; (b) explicit `App` + components for real apps; (c) seam-level
   access (custom backends, custom layout) for power users. Layer (a) must not
   prevent reaching layer (c).
4. **Explicit context, no globals.** Handles and parameters over singletons:
   test doubles are substituted by passing a different backend, not by flipping
   global switches.
5. **Fallible operations return `Result`; misuse fails loudly at build time
   where types allow.** Rendering a frame never panics on valid input; invalid
   usage (e.g. contradictory constraints) prefers compile-time prevention,
   then descriptive errors, never silent misrendering.
6. **Runtime-agnostic.** No public API takes or returns an async-runtime type,
   future executor, or runtime-specific handle. Blocking and event-driven use
   must both be expressible without naming Tokio, async-std, smol, or any other
   runtime.
7. **Docs are part of the API.** Every public item needs a doc example that a
   newcomer can read top-to-bottom; doc examples use the facade only and must
   be runnable against the test double.

## 2. Facade contract (`jilva`)

`jilva` is the single dependency users declare. Its surface rules:

- **Re-export only.** The facade defines (almost) no new types; it re-exports
  curated items from `jilva-core` and `jilva-terminal` under stable paths.
  Internal crate topology must never be visible in user code or error messages.
- **Flat where possible.** Two-level paths at most for everyday items
  (e.g. `jilva::App`, `jilva::events::KeyEvent` — path shapes illustrative).
  Deep module trees from internal crates are flattened at re-export.
- **Feature flags are additive and documented.** A default feature set yields
  the standard app (default backend included); flags may *add* capabilities or
  *swap* the backend, but disabling defaults must produce a clear compile-time
  story, not a broken facade. Exact flag names are implementation-RFC detail.
- **Prelude (if any) is tiny and optional.** Glob imports must never be
  required; any prelude contains only the handful of items used in every app.
- **Semver posture:** pre-`1.0`, minor versions may break with migration notes;
  patch versions never break. After `1.0`, standard semantic versioning. Any
  breaking change while in `0.x` still requires an RFC amendment so breakage
  stays deliberate (§3.1 of ARCHITECTURE.md).

## 3. App lifecycle API shape

The facade offers one obvious way to run an app, with explicit control underneath:

- A builder-style construction step (configure backend, initial state, options)
  separated from the run step (enter loop), so tests can construct without
  running and runners can inject the test double at construction.
- The run step owns terminal setup/teardown: entering alternate screen and raw
  mode on start, restoring everything on *all* exits (normal, error, panic —
  see consideration §8.7 of ARCHITECTURE.md). Users never call setup/teardown
  primitives directly in normal use.
- Shutdown is cooperative and expressive: the app signals intent to quit (and
  optionally an exit reason/result); the loop drains, tears down, and returns
  control with the terminal restored.
- Illustrative shape (non-normative sketch, does not compile):
  `build the app → run the loop → get a result back, terminal restored`.

## 4. Component model API shape

RFC-2 chooses the flavour; all flavours must satisfy this contract:

- **Identity:** the framework can tell "same component, new data" from "new
  component" across frames. Whether identity is positional, keyed, or explicit
  is RFC-2 detail — but the mechanism and its misuse behaviour must be
  documented and tested.
- **State ownership:** component-local ephemeral state (focus, scroll offset,
  entered text) is distinguished from app-owned domain state, with a stated
  rule for which lives where and how they communicate.
- **Event handling:** components declare which events they consume; unconsumed
  events propagate to parents by a documented bubbling/focus order. No
  component performs terminal I/O or mutates siblings.
- **Composition:** nesting children, lists of keyed children, and focus order
  are first-class concepts, not emergent accidents.
- **Purity of render:** producing a frame description has no observable side
  effects (no I/O, no global mutation), enabling golden-output tests.

## 5. Events and state propagation

- **Event taxonomy (stable, extensible):** keyboard (with progressive-enhancement
  ladder), mouse (click/drag/scroll, gated by terminal support), resize (new
  dimensions, always delivered), tick/time (for animation and polling), and a
  user-message channel for app-defined events. Adding a variant must not break
  exhaustive matches in user code without a major/migration path — the
  extensibility mechanism (non-exhaustive markers, wrapper enums) is RFC detail.
- **Dispatch order is documented and total:** every event has exactly one
  defined path (focused component → ancestors → app-level fallback), and
  focus ownership is always queryable.
- **State updates are transactional per event:** one event → update → request
  re-render. Intermediate states are never painted. The invalidation contract
  distinguishes layout-invalidating changes from paint-only changes
  (consideration §8.9 of ARCHITECTURE.md).

## 6. Error API

- One shared aggregate error type at the facade root with a stable taxonomy:
  backend/terminal I/O, capability/lifecycle, layout, render, and user-callback
  errors. The low-level backend API intentionally exposes `std::io::Result`;
  it does not expose backend-library-specific error types. A future runtime
  can convert those standard errors inward through `From<std::io::Error>`.
- Errors carry context (which operation, which component region where
  meaningful) and never leak backend-specific types through the facade.
- `panic!` appears only on violated internal invariants, with a message aimed
  at framework developers, and lifecycle teardown still attempts terminal
  restoration.

## 7. Style and theme API

- Components reference **semantic tokens** (`accent`, `muted`, `danger`,
  `selected` — names illustrative), never raw colours or ANSI attributes.
- A theme maps tokens to concrete attributes; the framework resolves them
  against the detected capability profile (truecolor → 256-colour → 16-colour
  degradation is automatic and snapshot-tested per tier).
- The curated default theme ships in the facade's default surface; custom themes
  override tokens selectively with the default filling gaps (no all-or-nothing
  theme authoring). Exact schema is RFC-3 detail.

## 8. Layout API

- Intent-based combinators for flex (row/column, grow/shrink, gap, alignment)
  and grid (tracks, placement, spanning). All units resolve to **integer cell
  rectangles**; fractional remainders distribute by a documented rule.
- Constraints are validated: over-constrained or unsatisfiable layouts produce
  a defined, documented behaviour (degrade gracefully, never panic, never
  overlap silently) — exact policy is RFC detail within boundary §3.6.
- Text measurement (graphemes, double-width) feeds layout as *inputs*; layout
  code never shapes text (ARCHITECTURE.md §3.6 rule).

## 9. Terminal backend seam in the API

- Current Phase-2 public seam: `Backend::{name, enter, leave, size, poll,
  read, submit, flush, capabilities}`, `CrosstermBackend` and
  `TestDoubleBackend`, re-exported through `jilva`. `enter`/`leave`,
  observations and emission use `std::io::Result`; `name` and the cached
  capability query do not. The full normative behavior is in
  `docs/PHASE2_BACKEND_CONTRACT.md`; frame/runtime APIs in earlier sections
  remain proposals for later phases.
- `poll(timeout)` reports source readiness; filtered unsupported events may
  make the next `read()` return `None`. `read()` also returns `None` for an
  empty queue. It preserves supported terminal events, including key kind and
  modifiers; optional mouse, focus and paste require explicit active support.
- `submit(&[OutputCommand])` validates and queues an entire ordered batch.
  Text output rejects terminal control characters; strings remain
  Unicode-capable and are never modeled as one scalar per cell. `flush()`
  attempts pending output once and discards that batch after the attempt.
  I/O errors are reported, but already-written terminal bytes cannot be
  rolled back. Cursor and nonempty clear rectangles must fit one cached screen
  size; flush rechecks geometry after a consumed resize.
- `TerminalCapabilities` and its color/input/screen domains are core-owned
  data. Detection occurs at session entry and queries use the cached snapshot.
  Dynamic refresh queue/frame policy is deferred to Phase 5. Resize updates
  screen geometry and its version, not unrelated domain versions.
- Users construct their backend explicitly in Phase 2. The default feature
  exposes `CrosstermBackend`; a future runtime builder is not implemented.
  Custom backends implement a small, documented interface (setup,
  teardown, read event, write instruction stream, report size/capabilities).
- The in-memory test double is a first-class, documented API for headless
  tests: scripted input events in, asserted typed instruction streams out.
  Frame snapshots and the app-construction builder remain later-phase work.
- Capability queries are explicit values users can read (to adapt content) but
  Phase 2 masks unsupported attributes and enforces monochrome policy at
  emission. Callers supply colors resolved for the snapshot's tier; semantic
  token/theme resolution and runtime adaptation belong to later milestones.

## 10. Naming and documentation conventions

- Rust API guidelines (naming, `Result`-returning verbs, builder patterns) apply
  throughout; deviations need justification in review.
- Public items: British-vs-American and abbreviation choices fixed once —
  `color` (matching the ecosystem convention), full words over abbreviations
  (`Selected` not `Sel`), event variants past-tense where they describe facts
  (`Resized`, `KeyPressed` — illustrative).
- Every public type documents: what it is, a minimal example, its error
  behaviour, and (for visual items) how it degrades at lower capability tiers.

## 11. Open questions (→ RFCs, not decided here)

1. Component flavour: retained/immediate/Elm-style hybrid? (RFC-2)
2. Exact theme schema and token list; default palette values. (RFC-3)
3. Tick/time source abstraction and frame-budget policy. (RFC-1/2)
4. Focus model details (single focus path vs. focus scopes). (RFC-2)
5. Whether effects (timers, background work) get a first-class API pre-`0.1.0`
   or stay user-side with a documented pattern. (RFC-1)
6. MSRV number and edition-following policy. (dedicated RFC per RFC process)
