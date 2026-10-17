# Architecture — jilva-rs

> **Status:** Phase 2 terminal engine in progress (revalidated 2026-10-17).
> This document defines system boundaries,
> crate structure, and dependency rules that implementation agents MUST follow.
> Anything marked *consideration* is a problem statement, not an implementation.
> Detailed API shapes are refined through the RFC process
> (`docs/RFC/README.md`); where this document and an accepted RFC disagree,
> the accepted RFC wins and this document must be amended.
>
> Related: `docs/API_DESIGN.md`, `docs/adr/`, `docs/ROADMAP.md`, `docs/RFC/`.

- Toolchain: Rust stable **1.98.1**, edition **2024**.
- License: **MIT only** — every crate, example, and bench in this workspace
  is MIT-licensed. No exceptions, no dual-license headers to maintain.
- MSRV policy itself is RFC-track (per `docs/RFC/README.md`); until that RFC
  lands, implementations must build on stable 1.98.1 and must not rely on
  nightly features.

## 1. Purpose and scope

`jilva-rs` is a terminal UI framework: components with calm defaults, intent-based
layout, tasteful motion, and correctness underneath (Unicode, truecolor, mouse and
keyboard, resize, testable rendering). This document answers three questions:

1. **What are the system boundaries?** (§3 — who owns what, who may call whom.)
2. **How is the code organised?** (§4–§7 — crates, dependency direction, file layout.)
3. **Which hard problems must future designs address?** (§8 — considerations,
   explicitly *not* implementations.)

Out of scope here: final public API signatures (see `docs/API_DESIGN.md` and the
RFCs), CI/build configuration, `Cargo.toml` contents, and any code.

## 2. Design principles

1. **Beauty as a feature, correctness underneath.** Defaults carry the taste;
   the foundations carry Unicode, input, resize, and restoration semantics.
2. **Small stable seams, free interiors.** Boundaries between crates are narrow,
   versioned, and documented. Interiors may evolve freely within semver.
3. **No ambient global state.** Terminal mode, styling context, and app state
   are passed explicitly so rendering stays testable without a real terminal.
   The real backend has one documented process-wide ownership lease because
   Crossterm input/raw modes are process resources. It carries no application
   data, capabilities, or backend selector; test doubles have no global lease.
4. **No async runtime in the core contract.** The framework must not force a
   choice of async runtime on its users (a README-level non-goal). Synchronous
   operation is the baseline; async integration, if any, is an opt-in layer
   above the core, decided by RFC.
5. **Testability is a boundary requirement.** Every boundary in §3 must be usable
   with a fake or in-memory stand-in: fake terminal, scripted events, fixed clock.
6. **Honest borrowing.** We learn from Ratatui, Cursive, Bubble Tea, and Textual
   without claiming superiority. Prior art is cited in RFCs, not dismissed here.

## 3. System boundaries

Each boundary lists its **responsibility**, what it **owns**, what it explicitly
**does not own**, and the **direction** other parts interact with it.

### 3.1 Public API facade

- **Responsibility:** the only crate users depend on directly. Re-exports the
  curated surface (`App`, component building blocks, events, style, layout
  helpers, error type) and hides internal crate topology.
- **Owns:** re-export list, feature-flag surface, prelude (if any), version
  number users see, semver promise.
- **Does not own:** any logic. No rendering, layout math, or terminal I/O lives
  here — it only re-exports.
- **Rule:** user-facing breaking changes require an RFC amendment, even in `0.x`,
  so they stay deliberate and documented.

### 3.2 App lifecycle

- **Responsibility:** own the run-loop contract: setup → event loop → teardown,
  including the guarantee that the terminal is restored on every exit path
  (normal return, `?`-propagated error, panic-unwind where feasible).
- **Owns:** lifecycle states and transitions, shutdown signalling, panic-guard
  strategy for terminal restoration (mechanism chosen at implementation RFC).
- **Does not own:** what the app *does* each frame (component model, §3.3), how
  pixels/cells reach the screen (renderer/terminal, §3.5/§3.7), or async runtimes.
- **Direction:** the lifecycle *drives* components and the renderer; neither
  drives it.

### 3.3 Component model

- **Responsibility:** define what a UI unit is: identity across frames,
  declared state, rendered output, and handled events.
- **Owns:** the component contract (capabilities a component must provide),
  identity semantics (how the framework knows "same component, new data"),
  composition rules (nesting, children, focus order).
- **Does not own:** application business state, persistence, or the diff/patch
  machinery (renderer concern, §3.7).
- **Note:** the exact contract (trait vs. struct+closure, Elm-style vs.
  immediate-mode vs. retained) is decided by RFC-2. This boundary only fixes
  *where* that decision lives and what it must specify: identity, state
  ownership, event handling, and composition.

### 3.4 State and event propagation

- **Responsibility:** define how data and events flow: events in (keyboard,
  mouse, resize, tick, custom messages), state updates, re-render requests.
- **Owns:** event types taxonomy, dispatch order, the invalidation contract
  ("what changed → what must be recomputed"), backpressure/queueing policy.
- **Does not own:** terminal decoding of raw bytes into events (terminal
  backend, §3.5) or widget-internal state.
- **Rule:** flow is unidirectional at the framework level (events → update →
  render). Components never write directly to the terminal or mutate sibling
  state; they emit intent that the loop reconciles.

### 3.5 Terminal abstraction

- **Responsibility:** isolate everything that touches a real terminal behind a
  narrow backend seam: enter/exit alternate screen and raw mode, query
  capabilities, read input events, write final cell/attribute streams, handle
  resize notifications.
- **Owns:** the backend capability interface, capability detection results,
  the in-memory test double.
- **Does not own:** application state, component trees, styling decisions, or
  layout. The backend receives fully-resolved draw instructions and decoded
  events; it never sees components (§3.3) or app state. (Rationale: ADR-0003.)
- **Rule:** `jilva-core` never names a terminal backend or library. The facade
  re-exports the selected terminal types without adding terminal I/O.
  Backend selection happens at the app-construction edge, by default the
  Crossterm-based backend (ADR-0003), replaceable by a test double with no
  production-code changes.

### 3.6 Layout

- **Responsibility:** turn intent ("row, gap 1, sidebar 30%") into concrete
  rectangles in **terminal cell geometry**: integer columns/rows, with full
  awareness that cells are not pixels.
- **Owns:** constraint vocabulary, the measure/arrange pass protocol, overflow
  and minimum-size policy, layout invalidation rules (which changes force
  re-layout vs. re-paint only).
- **Does not own:** text shaping, grapheme splitting, or styling.
- **Rule:** layout consumes *measured* sizes (already shaped text) and produces
  regions; it never measures text itself. A future Taffy integration must sit
  behind this seam and translate between CSS-style layout units and cell
  rectangles — the distinction is recorded in ADR-0003 and must not leak into
  component code.

### 3.7 Renderer

- **Responsibility:** turn a component tree + layout regions + styles into
  terminal output with minimal work: cell buffering, diffing against the
  previous frame, and emitting only what changed.
- **Owns:** the cell buffer model, the diff/patch algorithm choice,
  damage tracking (which regions are dirty), flush policy.
- **Does not own:** terminal I/O syscalls (backend, §3.5) or layout decisions
  (§3.6). It consumes regions and styled spans; it produces an instruction
  stream the backend executes.
- **Rule:** rendering is a pure function of (previous buffer, new frame
  description) plus an explicit capability profile — never of hidden globals —
  so golden-output tests run without a terminal.

### 3.8 Styling and theming

- **Responsibility:** turn theme intent ("accent", "muted", "danger") into
  concrete attributes (foreground/background, modifiers) resolved against the
  detected terminal capabilities (truecolor → 256 → 16 fallback chain policy).
- **Owns:** theme schema, token vocabulary, capability-degraded resolution
  rules, the curated default theme.
- **Does not own:** capability detection itself (backend reports, §3.5) or
  per-widget visual design (widget crate concern, future).
- **Rule:** components reference semantic tokens, never raw ANSI values.
  Snapshots test token resolution at each capability tier.

### 3.9 Error handling

- **Responsibility:** one coherent error story across the workspace.
- **Owns:** the facade error type and its taxonomy (I/O, backend, layout,
  render, user-callback), convertibility (`From` impls inward, never outward),
  and the guarantee that errors always unwind through lifecycle teardown (§3.2)
  so the terminal is restored.
- **Does not own:** recovery policy for user apps (retry? quit? log?) — the
  framework reports precisely and restores the terminal; the app decides.
- **Rules:** the shared `Error` is the runtime aggregate re-exported by the
  facade. The public low-level `Backend` seam intentionally returns
  `std::io::Result`; its errors may convert to `Error::Io` above that seam.
  Backend-library-specific errors do not leak into the facade.
  `panic!` is reserved for violated internal invariants,
  never for recoverable runtime conditions. Exact types are implementation-RFC
  detail; these rules are not negotiable.

### 3.10 Platform integration

- **Responsibility:** confine everything OS/terminal-specific: Unix/Windows
  console differences, capability detection, signal/resize plumbing, panic-hook
  interaction for restoration.
- **Owns:** the platform matrix we claim to support, detection strategy,
  documented degradation when capabilities are absent.
- **Does not own:** any framework logic beyond adapting to the platform.
- **Rule:** platform code lives only in the terminal backend crate (and its
  chosen backend dependency). `jilva-core` is platform-agnostic and must remain
  testable on any host without a TTY.

## 4. Crate structure

### 4.1 Initial crates (Phase 1 only)

| Crate | Role | Contains |
|---|---|---|
| `jilva` | Public facade (§3.1) | Re-exports, feature flags, docs entry point. No logic. |
| `jilva-core` | Framework foundations | App lifecycle, component contract, state/event propagation, layout protocol + region types, renderer buffer/diff model, style token model, facade error taxonomy (as internal types re-exported via facade). Platform-agnostic. |
| `jilva-terminal` | Backend abstraction (§3.5 + §3.10) | Backend seam interface, capability model + detection, default backend implementation, in-memory test double. Only crate allowed to touch terminal I/O or depend on a terminal library. |

### 4.2 Future crates — integration points only (MUST NOT be created in Phase 1)

These are reserved names with defined seams so Phase 1 designs do not block
them. No code, no stubs, no empty crates.

| Reserved name | Seam it will plug into |
|---|---|
| `jilva-render` | Extracted from core renderer model (§3.7) if/when the buffer/diff machinery outgrows core. Seam: consumes layout regions + styled spans, emits backend instruction streams. |
| `jilva-layout` | Extracted from core layout protocol (§3.6) if/when constraint solving (e.g. Taffy) outgrows core. Seam: intent in, cell rectangles out. |
| `jilva-style` | Extracted from core style model (§3.8) with theme schema and curated themes. Seam: tokens in + capability profile in, attributes out. |
| `jilva-widgets` | Component library (text, list, table, input, tabs, status bar per README scope). Seam: implements the §3.3 component contract only; no direct terminal/layout internals. |
| `jilva-motion` | Tasteful transitions/live updates. Seam: a time-scoped transform over frame descriptions feeding the renderer; must degrade to static output. |
| `jilva-devtools` | Inspector, golden-record tooling. Seam: read-only observer of event log + frame stream; never in the hot path. |
| `jilva-cli` | Scaffolding/clap-level helpers. Seam: depends on the facade only, like any user app. |

Extraction rule: a future crate is created only when its seam (§3) is already
stable and an RFC shows the code inside `jilva-core` has outgrown a module.
Extraction must not change the facade surface.

## 5. Crate-vs-module rationale

- **Facade (`jilva`) is a crate, not a module**, because semver and dependency
  hygiene need a hard line: users depend on exactly one artifact, and internal
  topology can change without breaking them. A module cannot provide that.
- **Terminal backend is a crate, not a module**, because platform I/O,
  third-party terminal dependencies, and capability detection form a distinct
  failure and versioning domain. Containment limits the blast radius of backend
  upgrades and keeps `jilva-core` TTY-independent and fast to test.
- **Everything inside `jilva-core` stays a module for now** (lifecycle,
  component, events, layout, renderer model, style): these domains co-evolve
  rapidly pre-`0.1.0`, and premature crate splits would freeze immature seams
  and slow RFC iteration. Modules keep refactoring cheap while §3 boundaries
  keep responsibilities clear. Split only via the extraction rule (§4.2).
- **Widgets, motion, devtools, CLI are future crates, not core modules**,
  because they change for different reasons and on different cadences than the
  foundations, and several (motion, devtools) must be optional dependencies
  users can exclude.

## 6. Dependency direction rules

The allowed dependency graph (arrows mean "may depend on"):

```text
user app  →  jilva  →  jilva-core
                  ↘→  jilva-terminal  →  (terminal library, e.g. Crossterm)
                         (terminal ──type-only──▶ core, see rule 1)
```

1. **No cycles, ever.** `jilva-core` depends on nothing in this workspace.
   `jilva-terminal` depends on `jilva-core` for shared capability profiles,
   events, output commands, geometry and their pure validation/value helpers.
   It never decides application, component, layout or rendering behavior.
   It may *pump* a core-defined `Driven` object (RFC-0008 §4): read input
   from a backend, pass it with the elapsed time to `Driven::handle`, and
   write the returned output commands. Dispatch, update, view, layout and
   paint stay inside core; the driver owns only the terminal session, the
   wall clock and the event-pump loop.
   `jilva-core` never depends on `jilva-terminal`; `jilva` depends on both.
   This is the 2026-10-16 ownership amendment, clarified 2026-10-17 to remove
   the contradictory original no-dependency sentence, and clarified again on
   2026-10-17 by the project owner (RFC-0008 open question 1) to place the
   run driver in `jilva-terminal`.
2. **Core never names a backend.** No terminal library, no platform module, no
   `cfg(windows)` branching inside `jilva-core`. Capability *profiles* are plain
   data defined in core; capability *detection* lives in `jilva-terminal`.
   Pure helpers may interpret injected values in core; reading the process
   environment, querying a TTY and deciding available terminal modes belong
   to terminal. Core performs no ambient probing.
3. **Terminal never sees app state.** `jilva-terminal` operates on draw
   instructions and emits decoded events. It takes no component, state, or
   style-token types as inputs (ADR-0003). The driver's generic `D: Driven`
   parameter is opaque: terminal code calls only the `Driven` methods and
   never names, inspects or constructs the application model.
4. **Facade contains no logic.** If a pull request adds behaviour to `jilva`
   that could live in core or terminal, it belongs there.
5. **Future crates obey the same acyclicity:** `jilva-widgets` → facade-level
   contracts only; `jilva-motion`/`jilva-devtools` observe frame/event streams,
   never internals; `jilva-cli` is a downstream consumer like any app.
6. **External dependencies are minimal and justified per crate.** Each new
   third-party dependency needs a stated reason in its RFC/PR; the facade adds
   none beyond its two workspace members plus explicitly user-facing opt-ins.
   `jilva-core` carries `unicode-width` and `unicode-segmentation` (the width
   and segmentation authorities, RFC-0004 §4), confined to
   `renderer/width.rs`.

## 7. Workspace file layout contract

Implementation agents MUST produce exactly this layout — no extra top-level
crates, no `src/` at the workspace root, no renamed directories:

```text
jilva-rs/
  Cargo.toml                  # workspace manifest ONLY (members + shared lints);
                              # owned by the implementation agent, not this doc
  LICENSE                     # MIT (exists — do not touch)
  README.md / SECURITY.md / CONTRIBUTING.md   # exist — other agents own changes
  docs/
    ARCHITECTURE.md           # this file
    API_DESIGN.md             # API principles and facade contract
    ROADMAP.md                # exists
    RFC/                      # exists (process + proposals)
    adr/
      0001-rust-first.md
      0002-workspace-structure.md
      0003-terminal-backend-strategy.md
  crates/
    jilva/                    # facade; re-exports + docs only
      src/lib.rs
    jilva-core/               # foundations; modules mirror §3 boundaries:
      src/
        lib.rs                # module declarations + crate docs only
        lifecycle.rs          # §3.2  (module names are illustrative of the
        component.rs          # §3.3   boundary, not a fixed item list —
        events.rs             # §3.4   RFC-2 may refine within the boundary)
        capabilities.rs       # shared capability domains and pure helpers
        output.rs             # validated typed output commands
        layout.rs             # §3.6
        renderer.rs           # §3.7
        style.rs              # §3.8
        error.rs              # §3.9
  crates/jilva-terminal/      # backend seam + default backend + test double
      src/
        lib.rs
        backend.rs            # the seam interface (§3.5)
        capabilities.rs       # environment/probe detection (§3.10)
        crossterm_backend.rs  # terminal I/O and resource state machine
        test_double.rs        # in-memory backend for tests (name negotiable)
  examples/                   # one tiny example per concept; facade-only imports
  tests/                      # workspace integration tests (golden-output, no TTY)
  benches/                    # frame/diff micro-benchmarks (std-only, wired via
                              # [[bench]] in the facade; first: render_diff)
```

Conventions: `examples/` and `tests/` import only the `jilva` facade (plus the
test double via the facade's test surface) — never `jilva-core` or
`jilva-terminal` directly. This keeps the facade honest: anything an example or
integration test needs must be reachable through it.

> **Wiring note (virtual workspace).** The workspace root is virtual, not a
> Cargo target, so root-level `examples/` and `tests/` files would never
> compile if left unwired. They are therefore declared explicitly in
> `crates/jilva/Cargo.toml` via `[[example]]` / `[[test]]` sections with
> `path = "../../examples/…"` / `path = "../../tests/…"`. The contract paths
> stay at the repo root; only the manifest wiring lives in the facade crate.

## 8. Architectural considerations (NOT implementations)

The following are problem statements future RFCs and code MUST address. They
constrain designs; they prescribe no data structures, algorithms, or code.

1. **Cell buffering.** Frames are described as grids of cells (grapheme +
   attributes per cell) held in memory so consecutive frames can be compared.
   Designs must state buffer ownership, coordinate bounds discipline, and what
   "empty vs. styled-blank" means for diffing.
2. **Differential rendering.** Only changed cells/attributes cross the backend
   seam. Designs must state the diff granularity, how cursor movement is
   minimised, and the worst-case cost of pathological frames (full-screen churn).
3. **Unicode grapheme clusters.** A user-perceived character may span multiple
   scalar values and multiple bytes but occupies a layout width decided by
   shaping, not by byte or `char` count. Designs must state where segmentation
   happens and which authority defines widths.
4. **Double-width and emoji.** CJK and emoji occupy two cells (or vary by
   terminal); truncation, wrapping, and cursor math must account for this or
   columns drift. Designs must state the width table source and the fallback
   when the terminal disagrees.
5. **Resize.** Terminal size can change mid-frame. Designs must state who
   observes the resize, whether in-flight frames are dropped or completed, and
   the re-layout → re-render ordering guarantee.
6. **Keyboard and mouse.** Key encodings differ across terminals (legacy vs.
   Kitty-protocol-style progressive enhancement); mouse reporting modes vary.
   Designs must state the supported encoding ladder and the degradation when a
   terminal offers less.
7. **Lifecycle and terminal restoration.** Alternate screen, raw mode, cursor
   visibility, and mouse reporting must be restored on *every* exit path.
   Designs must enumerate normal, error (`?`), and panic paths and state what
   is guaranteed on each.
8. **Component identity.** Reconciliation needs stable identity across frames
   ("same list, scrolled" vs. "new list"). Designs must state the identity
   mechanism and its collision/misuse behaviour.
9. **Layout invalidation.** Re-layout is expensive; re-paint is cheap. Designs
   must state the dirty-propagation rule: which changes invalidate layout vs.
   paint only, and how containers bound invalidation cascades.
10. **Minimal redraw.** Damage tracking must compose with diffing: a small state
    change in a large static screen must not re-emit the world. Designs must
    state how damage regions flow from state changes to the backend.
11. **Performance posture.** Target behaviour (to be quantified by RFC and
    benches): full-screen diff within a frame budget at typical sizes, bounded
    allocation in the hot path, no pathological quadratic behaviour on wide
    screens. Numbers belong to RFCs/benches, not to this document.
12. **Optional async.** If async event sources or effects are ever supported,
    they sit *above* the synchronous core as an opt-in bridge with explicit
    scheduling semantics, runtime-agnostic. The core loop must remain usable
    with zero async dependencies.
13. **Testability without a real terminal.** Golden-output tests (frame in →
    expected instruction stream out), scripted event sequences, and fixed-size
    virtual screens are first-class. Designs must show how each boundary is
    exercised headless.
14. **Cross-platform capability detection.** Capability profiles (truecolor,
    unicode support level, mouse, progressive key encoding, alternate screen)
    are detected at startup, cached, and threaded explicitly into style
    resolution and rendering — never sniffed ad hoc mid-frame.

## 9. Cross-cutting notes

- **Error handling** (§3.9) and **platform integration** (§3.10) are full
  boundaries with owners, not afterthoughts: every RFC touching rendering,
  input, or lifecycle must state its error taxonomy impact and its platform
  behaviour.
- **No superiority claims.** This architecture positions Jilva as one more
  experiment in a lineage (Ratatui, Cursive, Bubble Tea, Textual) with a
  specific bet — defaults, docs, and craft — not as a replacement. Comparisons
  in RFCs must be concrete and falsifiable.

## 10. What this document does NOT decide

- Exact public API signatures (→ `docs/API_DESIGN.md` + RFC-1/2/3).
- The component contract flavour (→ RFC-2).
- The theme schema and default theme (→ RFC-3).
- Backend dependency version, MSRV number, lint set, CI shape (→ RFC/implementers).
- Any algorithm, data structure, or code.

## 11. Conformance checklist for implementers

- [ ] Layout in §7 is followed exactly; no new top-level crates.
- [ ] Dependency rules in §6 hold; verifiable by crate dependency inspection.
- [ ] `jilva-core` builds and its tests pass on a machine with no TTY.
- [ ] `examples/` and `tests/` use the facade only.
- [ ] Every §8 consideration is referenced by at least one RFC or explicitly
      deferred with a tracking issue — none are silently dropped.
- [ ] No claim of superiority over other frameworks appears in docs or messages.
