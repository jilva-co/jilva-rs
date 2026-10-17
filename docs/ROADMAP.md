# Roadmap — jilva-rs

No dates. Phases move when the work is honestly done.

## Phase 1 — Foundation (complete 2026-10-16)

Workspace scaffolding and design groundwork. No engine, renderer, or widgets.

- [x] Org profile and community files (`jilva-co/.github`)
- [x] Repo vision (this repo's README)
- [x] Architecture boundaries and crate structure (`docs/ARCHITECTURE.md`)
- [x] API principles and facade contract (`docs/API_DESIGN.md`)
- [ ] Cargo workspace exactly per `docs/ARCHITECTURE.md` §7
  (`crates/jilva`, `crates/jilva-core`, `crates/jilva-terminal`,
  `examples/`, `tests/`, `benches/`; no root `src/`, no extra crates)
- [x] RFC process published (`docs/RFC/README.md` + template)
  (governance amended 2026-10-16: maintainer-driven approval, external
  7-day wait waived by project owner — see `docs/RFC/README.md`)
- [x] RFC-1: scope and non-goals — ACCEPTED 2026-10-16
- [x] RFC-2: component model — ACCEPTED 2026-10-16
- [x] RFC-3: theming — ACCEPTED 2026-10-16
  (decisions + rationale + deferred items:
  `docs/reviews/RFC_DECISION_TRACKER.md`)
- [x] Contributor setup and workspace workflow docs
  (`docs/GETTING_STARTED.md`, `docs/DEVELOPMENT.md`)

Exit criterion: the workspace builds cleanly, facade-only conventions hold for
`examples/` and `tests/`, and RFC-1/2/3 are accepted. — MET 2026-10-16
(acceptances recorded; workspace-verification box above remains with the
implementation track).

## Phase 2 — Terminal Engine (started 2026-10-16, in progress 2026-10-17)

The backend seam behind which all real-terminal contact lives
(`docs/ARCHITECTURE.md` §3.5, §3.10).

Current inspection: a trait, shared event/output/capability types, Crossterm
implementation and scripted test double already exist in the working tree.
Their presence is not an exit verification. The 2026-10-17 continuation
revalidates governance and corrects backend semantics; implementation and
independent audit continue. See `docs/reviews/RFC_ACCEPTANCE_CHECKLIST.md`.

- [x] Current authorization, technical acceptance and stable seam decisions
  recorded with historical evidence limitations (2026-10-17).
- [ ] Implementation behavior verified against the corrected backend contract.
- [ ] Required Cargo checks, tests, formatting, Clippy and strict rustdoc pass.
- [ ] Linux, Windows and macOS CI outcomes observed and reported accurately.
- [ ] Minimal terminal demo builds; interactive/PTY validation reported
  separately from headless tests.
- [ ] Independent terminal-engine audit has no unresolved blocker.

Keep Phase 2 in progress until its actual verification and platform evidence
are recorded. A configured CI matrix or an opened PR is not a successful run.

- Backend capability interface plus capability model and detection
  (truecolor, unicode support level, mouse, progressive key encoding,
  alternate screen)
- Default backend implementation (Crossterm-based per ADR-0003)
- In-memory test double as a first-class, documented API for headless tests
- Platform matrix: Unix/Windows console differences, resize plumbing,
  documented degradation when capabilities are absent

Exit criterion: golden-output and scripted-event tests run with no TTY;
`jilva-core` names no backend and contains no platform branching.
Implementer contract: `docs/PHASE2_BACKEND_CONTRACT.md` (normative for
Phase-2 work; backend selection/crossterm wiring belongs to the
implementation agent).

## Phase 3 — Rendering Engine (planned)

Cell buffering, diffing, and damage tracking (`docs/ARCHITECTURE.md` §3.7;
considerations §8.1, §8.2, §8.10, §8.11).

- Cell buffer model: ownership, coordinate bounds, empty-vs-styled-blank
  semantics for diffing
- Diff/patch machinery: granularity, cursor-movement minimisation,
  worst-case cost of full-screen churn
- Damage tracking composed with diffing: small state changes must not
  re-emit the world
- Rendering stays a pure function of (previous buffer, new frame
  description) plus an explicit capability profile — no hidden globals

Exit criterion: golden-output tests (frame in → expected instruction stream
out) cover the buffer/diff/damage behaviour; performance posture is
quantified by RFC and benches, not adjectives.

Phase-3 exit obligation (tracked, from RFC-1 Unicode row + Phase-1.7
audit): the provisional `Cell { symbol: char }` model MUST be replaced by
the grapheme-capable symbol type + width/segmentation authorities no later
than this phase — golden tests must not entrench a `char`-shaped API
(RFC-2 §8 dependency, RFC-1 OQ12).

## Phase 4 — Layout & Components (planned)

Intent-based layout and the component contract (`docs/ARCHITECTURE.md` §3.3,
§3.6; considerations §8.3, §8.4, §8.8, §8.9). Exact contract flavour is
decided by RFC-2.

- Constraint vocabulary (flex row/column, grid tracks, gap, alignment) resolving
  to integer cell rectangles with a documented fractional-remainder rule
- Measure/arrange pass protocol; layout consumes measured (shaped) sizes and
  never shapes text itself
- Overflow and minimum-size policy; over-constrained layouts degrade
  gracefully — never panic, never overlap silently
- Component contract: identity across frames, ephemeral-vs-app state ownership,
  event handling without terminal I/O or sibling mutation, composition
  (nesting, keyed children, focus order)
- Text shaping inputs: grapheme segmentation and double-width/emoji width
  handling feed layout as inputs, with a stated width-table authority

Exit criterion: RFC-2 accepted and reflected in the layout/component modules;
layout invalidation rules distinguish re-layout from re-paint-only changes.

## Phase 5 — State & Interactivity (planned)

Event taxonomy, dispatch, and the update→render loop (`docs/ARCHITECTURE.md`
§3.2, §3.4; considerations §8.6, §8.7, §8.12).

- Event types: keyboard (with progressive-enhancement ladder), mouse
  (click/drag/scroll, gated by terminal support), resize (always delivered),
  tick/time, user-message channel
- Documented, total dispatch order (focused component → ancestors →
  app-level fallback); focus ownership always queryable
- Transactional updates: one event → update → requested re-render;
  intermediate states are never painted
- Lifecycle guarantees: terminal restoration (alternate screen, raw mode,
  cursor, mouse reporting) on every exit path — normal, error (`?`), panic
- Async, if ever supported, sits above the synchronous core as an opt-in,
  runtime-agnostic bridge; the core loop stays usable with zero async
  dependencies

Exit criterion: scripted event sequences drive apps headlessly through the
test double; every exit path is covered by a restoration test.

## Phase 6 — Design System (planned)

Theme schema, token vocabulary, and the curated default theme
(`docs/ARCHITECTURE.md` §3.8). Exact schema is decided by RFC-3.

- Components reference semantic tokens, never raw ANSI values
- Theme maps tokens to attributes; resolution against the capability profile
  (truecolor → 256 → 16 fallback chain) is automatic and snapshot-tested
  per tier
- Custom themes override tokens selectively; the default fills gaps
  (no all-or-nothing theme authoring)
- Curated default theme ships in the facade's default surface

Exit criterion: RFC-3 accepted; token-resolution snapshots pass at each
capability tier.

## Phase 7 — Alpha Release `0.1.0` (planned)

- Minimal core: run loop, layout, theming, essential widgets
  (text, list, table, input, tabs, status bar)
- `cargo add jilva` works; docs site section; examples folder
- Breaking changes expected through `0.x` (each still requires an RFC
  amendment so breakage stays deliberate); patch versions never break
- `1.0` only when the API is stable and proven in real apps
- Sibling languages (`jilva-go`, `jilva-ts`) start only after the Rust alpha
  is genuinely usable

Prototype policy (applies throughout): throwaway branches may test RFC ideas
against reality at any phase. Prototype code is never published to crates.io,
may be discarded entirely, and never substitutes for an RFC. Success criterion:
RFCs survive contact with code, or get rewritten.

## Milestones (GitHub)

- `Foundation` — Phase 1 items
- `RFC-1 scope`, `RFC-2 components`, `RFC-3 theming`
- `Terminal engine`, `Rendering engine`, `Layout & components`,
  `State & interactivity`, `Design system`
- `v0.1-alpha` — created when Phase 7 begins, not before
