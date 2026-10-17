# Roadmap — jilva-rs

No dates. Phases move when the work is honestly done.

## Phase 1 — Foundation (complete 2026-10-16)

Workspace scaffolding and design groundwork. No engine, renderer, or widgets.

- [x] Org profile and community files (`jilva-co/.github`)
- [x] Repo vision (this repo's README)
- [x] Architecture boundaries and crate structure (`docs/ARCHITECTURE.md`)
- [x] API principles and facade contract (`docs/API_DESIGN.md`)
- [x] Cargo workspace per `docs/ARCHITECTURE.md` §7
  (`crates/jilva`, `crates/jilva-core`, `crates/jilva-terminal`,
  `examples/`, `tests/`; `benches/` reserved until benchmarks exist;
  no root `src/`, no extra crates). Rechecked 2026-10-17.
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
(historical acceptance record preserved; workspace and current acceptance
authorization revalidated 2026-10-17).

## Phase 2 — Terminal Engine (implemented and verified 2026-10-17)

The backend seam behind which all real-terminal contact lives
(`docs/ARCHITECTURE.md` §3.5, §3.10).

The existing implementation was audited and completed: a synchronous optional
Crossterm backend, typed events/output/capabilities, retryable resource cleanup,
deterministic headless backend, and separate real/headless demos. Verification
is recorded in `docs/reviews/PHASE2_CONTINUATION_REPORT.md`; this status does
not imply publication, runtime, renderer, or a widget library. The implementation
PR #8 was merged externally by @hamroqulovv while this continuation was still
preparing its final documents; the agent did not issue a merge command.

- [x] Current authorization, technical acceptance and stable seam decisions
  recorded with historical evidence limitations (2026-10-17).
- [x] Implementation behavior verified against the corrected backend contract.
- [x] Required Cargo checks, tests, formatting, Clippy and strict rustdoc pass.
- [x] Linux, Windows and macOS CI outcomes observed and reported accurately.
- [x] Minimal terminal demo builds; interactive/PTY validation reported
  separately from headless tests.
- [x] Independent terminal-engine audit has no unresolved blocker.

Evidence: 111 headless unit/integration tests and 27 doctests pass locally;
the opt-in Windows ConPTY normal/panic restoration test passes separately.
Ubuntu, Windows and macOS quality jobs plus formatting pass in
[CI run 39000026013](https://github.com/jilva-co/jilva-rs/actions/runs/39000026013)
for `9fef589`. Earlier runs failed formatting; their correction and results
are retained in the continuation report. PR #8 was merged externally at
2026-10-17T03:47:19Z as `442d9d8`. Audit and delivery documentation are supplied
on a separate follow-up branch for review. Physical terminal coverage remains limited
as documented; headless CI is not visual validation.

- Backend capability interface plus capability model and detection
  (truecolor/256/ANSI16/mono tiers, mouse, reported key kinds,
  alternate screen)
- Default backend implementation (Crossterm-based per ADR-0003)
- In-memory test double as a first-class, documented API for headless tests
- Platform matrix: Unix/Windows headless verification, resize plumbing,
  documented optional-mode and Unicode-width limitations

Exit criterion: golden-output and scripted-event tests run with no TTY;
`jilva-core` names no backend and contains no platform branching.
Implementer contract: `docs/PHASE2_BACKEND_CONTRACT.md` (normative for
Phase-2 work; backend selection/crossterm wiring belongs to the
implementation agent).

## Phase 3 — Rendering Engine (implemented on branch `phase3-rendering-engine`; RFC-0004 Draft)

Implemented in `jilva-core::renderer` per `docs/RFC/0004-rendering-engine.md`;
not yet merged, and RFC-0004 awaits a maintainer decision.

- [x] Cell buffer model: owned `Buffer`, clipped writes, wide-glyph pairing
  invariant, empty-vs-styled-blank semantics
- [x] Grapheme-capable `Cell` replacing `Cell { symbol: char }`, with named
  width (`unicode-width`, no `cjk`) and segmentation (`unicode-segmentation`)
  authorities and a truncation rule that never splits a cluster or wide cell
- [x] Diff/patch: coalesced segments, attribute tracking, cursor-move
  minimisation with gap bridging, capability-effective equality
- [x] Damage tracking composed with diffing (`Damage`, wide-glyph widening)
- [x] Golden-stream tests plus a randomised round-trip property test against a
  virtual terminal; facade integration tests against the headless backend
- [x] First benchmark (`benches/render_diff.rs`); numbers in RFC-0004
- [ ] Maintainer decision on RFC-0004; CI observation on all three platforms;
  real-terminal visual validation (headless tests are not visual validation)

Original scope, for reference:

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
quantified by RFC and benches, not adjectives. — Implemented; pending the
maintainer decision and CI evidence listed above.

Phase-3 exit obligation (tracked, from RFC-1 Unicode row + Phase-1.7
audit; discharged by RFC-0004 §3–§4 on the Phase-3 branch): the provisional `Cell { symbol: char }` model MUST be replaced by
the grapheme-capable symbol type + width/segmentation authorities no later
than this phase — golden tests must not entrench a `char`-shaped API
(RFC-2 §8 dependency, RFC-1 OQ12).

## Phase 4 — Layout & Components (in progress)

First slice implemented on branch `phase4-layout-solver` (stacked on the
Phase-3 branch; RFC-0005 Draft, `docs/RFC/0005-layout-solver.md`):

- [x] Flex row/column solver: closed constraint vocabulary (`Length`,
  `Percentage`, `Min`, `Max`, `Fill`), gap, alignment, named remainder rule
  (largest fraction, earlier item wins ties), never-panic overflow policy
  (later items yield first); `Rect::{right, bottom, intersection, inner}`
- [x] Owned `View` description + paint pass + `Renderer::render` (RFC-0006
  Draft, `docs/RFC/0006-view-description.md`): text/spans, row/column,
  overlay, padding, borders; infallible clipped paint
- [x] Colour blending and motion (RFC-0007 Draft): `ResolvedColor::blend`,
  `Buffer::map_attrs`, `View::{gradient, gradient_text, tint}`, `Easing`,
  `Lerp`, `Transition` (injected time, retarget, reduced motion) — the first
  features of the beauty-first direction in `docs/adr/0004-product-direction.md`
- [ ] Grid tracks, measure/arrange protocol (intrinsic sizing), text wrapping
- [ ] App runtime and component contract (RFC-2 reflected in code),
  invalidation wiring — planned in RFC-0008 Draft
  (`docs/RFC/0008-app-runtime.md`): slice A counter app, slice B two
  focusable counter components
- [ ] Agent-chat dogfood app (transcript, streaming text, prompt input,
  tool-call card, approval prompt), offline and scripted, driving the
  remaining widgets (ADR-0004, amended 2026-10-17; replaces the dashboard)
- [ ] Maintainer decisions on RFC-0005, RFC-0006, RFC-0007 and RFC-0008

Original scope, for reference:

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
