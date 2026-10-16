# RFC-0001: Framework Scope, Principles, Non-Goals

- Status: Draft
- Author(s): Subagent 1 — Product Scope & Framework Strategy Architect (Jilva Phase 1.5 RFC sprint)
- Created: 2026-10-16
- Revised: 2026-10-16 (Round 2 cross-review against RFC-2, RFC-3, ARCHITECTURE_FEASIBILITY.md — status stays Draft)
- Related: docs/ARCHITECTURE.md, docs/API_DESIGN.md, docs/adr/0001-rust-first.md, docs/adr/0002-workspace-structure.md, docs/adr/0003-terminal-backend-strategy.md, docs/ROADMAP.md (Phase 1 → Phase 7)

> Lifecycle note: this RFC is **Draft**. Per `docs/RFC/README.md` it must pass
> through **Discussion (minimum 1 week)** before any decision of
> Accepted / Rejected / Postponed. Nothing in this document is accepted
> until a maintainer merges it as such. Amendments after acceptance are new
> short RFCs.

## Summary

This RFC fixes what `jilva-rs` **is and is not** before any engine,
renderer, or widget code exists. It declares `jilva-rs` a **terminal UI
framework** (not a widget library, not a GUI toolkit, not an application
framework), draws the line between **framework-owned and app-owned**
responsibilities, defines the **public vs. internal API** rule, and states
the **alpha-essential scope** for `0.1.0` with explicit postponements and
permanent refusals. It is the scope parent for RFC-2 (component model) and
RFC-3 (theming): any scope conflict between this RFC and
`docs/ARCHITECTURE.md` is flagged as an Open Question, never silently
overridden. One paragraph for users: if you `cargo add jilva` at `0.1.0`,
you get one facade crate with a run loop, intent-based layout, semantic
theming with graceful degradation, and a small set of beautiful-by-default
widgets — everything else is a documented postponement or refusal.

## Motivation

Every TUI project eventually faces the same scope fights: "is image display
in scope?", "do we need our own async runtime?", "why can't the framework
also do config/networking?", "can we just expose ANSI and let apps style
themselves?". Without a written scope, each fight is re-litigated per PR and
the framework accretes unrelated features until its API is unlearnable.
Concrete stories:

1. A contributor proposes Sixel/Kitty image support "because Textual and
   OpenTUI-adjacent tools discuss images". Without a scope table, reviewers
   argue from taste; with one, the answer is "Could — post-alpha, behind the
   backend seam, RFC required".
2. A user asks for Tokio integration in core "because Textual has workers".
   The scope answer must be structural (ARCHITECTURE.md §2.4: no async
   runtime in the core contract), not personal.
3. A widget PR adds raw-ANSI styling "for flexibility". API_DESIGN.md §7
   already forbids it (semantic tokens only); this RFC makes that a
   Must-level scope rule with rationale.

This RFC exists so RFC-2 and RFC-3, and every Phase 2–7 implementation, have
a bounded field to play in.

## Proposal

### 1. The 14 required scope answers

**Q1 — Framework vs. widget library: what is `jilva-rs`?**
A **framework**: it owns the run-loop contract (setup → event loop →
teardown with guaranteed restoration), the component contract, the
event/state propagation rules, the layout protocol, the renderer
buffer/diff model, the style-token model, and the backend seam. A widget
library (e.g. a set of `Paragraph`/`Table` render helpers with no loop, as
Ratatui's core historically positioned itself — see Prior art) leaves the
loop, state ownership, and restoration to the user. Jilva owns the loop;
that ownership is what makes "beautiful by default" and "restoration on
every exit path" enforceable rather than advisory. Component-contract flavour:
RFC-2 (Draft, not accepted) proposes a hybrid — Elm/MVU loop + retained
identity (paths/keys) + pure view; the choice stays open until RFC-2 is
accepted, and the Round-2 cross-review record logs where RFC-2 diverges
from this scope (notably change-class taxonomy).

**Q2 — What does the framework own vs. what do apps own?**
Framework owns: lifecycle states and transitions, shutdown signalling,
panic-guard restoration strategy; event taxonomy, dispatch order, and the
invalidation contract (layout-invalidating vs. paint-only); constraint
vocabulary and measure/arrange protocol; cell-buffer/diff/damage model;
theme schema, token vocabulary, and capability-degraded resolution; the
facade error taxonomy; capability detection strategy and platform matrix.
Apps own: domain state, persistence, business logic, recovery policy
(retry/quit/log), networking, configuration, logging setup, choice of async
runtime (if any). Components never perform terminal I/O or mutate sibling
state; they emit intent the loop reconciles (ARCHITECTURE.md §3.4 rule).
The framework reports errors precisely and restores the terminal; the app
decides what recovery means (ARCHITECTURE.md §3.9).

**Q3 — Public vs. internal API: what is the rule?**
One public crate: `jilva` (facade, re-exports only, no logic).
`jilva-core` and `jilva-terminal` are internal implementation detail: their
paths must never appear in user code, user imports, or facade error
messages. Internal crates keep their own error types with `From`
conversions inward; only the facade error type is public (ARCHITECTURE.md
§3.9, API_DESIGN.md §2/§6). The never-leak rule covers solver/engine types
as well as backend types: over-constrained layouts map solver errors into
the facade taxonomy, never exposing a layout engine's error enum through
`jilva::Error` (feasibility §4 error-DX requirement). Feature flags are additive and documented; the
default feature set yields the standard app with the default backend.
User-facing breaking changes require an RFC amendment even in `0.x`
(ARCHITECTURE.md §3.1 rule). `examples/` and `tests/` import only the
facade (plus the test double via the facade's test surface) — this is the
mechanical honesty check on the facade.

**Q4 — Alpha essentials: what is the smallest `0.1.0` that is still Jilva?**
Minimal core (ROADMAP Phase 7): run loop with restoration guarantees;
layout (flex row/column + grid, gap, alignment, documented remainder rule);
theming (token vocabulary + curated default theme + truecolor → 256 → 16
→ mono automatic resolution per Q12/RFC-3 §5); essential widgets **text, list, table, input, tabs,
status bar**; keyboard + resize + tick + user-message events; mouse
click/scroll where the terminal supports it with documented degradation;
in-memory test double as first-class API; golden-output and scripted-event
tests headless. Anything in this paragraph missing at alpha means the
release is not `0.1.0` — it is a pre-alpha milestone.

**Q5 — What is explicitly postponed (not refused)?**
Post-alpha, each requiring its own RFC with prototype evidence: additional
widgets (tree, charts/sparklines, modal/dialog, toast, scrollbar,
scrollable containers); grid spanning refinements; focus scopes (see Q7);
animation/motion crate (`jilva-motion` as time-scoped frame transform,
degrading to static); devtools crate (inspector, golden-record tooling,
read-only observer, never hot-path); scaffolding/CLI helpers (`jilva-cli`,
facade-only consumer); images/graphics protocols (Sixel, Kitty, iTerm2
inline, half-block fallbacks); remote/multiplexed-session optimisations;
suspend/resume and shell-out support; clipboard (OSC 52) integration;
selection/clipboard ownership semantics. Postponed ≠ promised: each needs an
RFC that re-argues cost at that time.

**Q6 — Complexity risks: where can this framework die of cleverness?**
(a) Layout-engine leakage: exposing CSS-pixel/float semantics (Taffy/Yoga
directly) into a cell-addressed medium — refused by ADR-0003; any engine
sits behind the layout seam with explicit unit translation.
(b) Backend-aware core: letting components/styles leak into
`jilva-terminal` for "optimisation" — refused by ADR-0003; optimisation
belongs in the diff/patch layer on instruction streams.
(c) Premature crate splits: freezing immature seams pre-`0.1.0` — refused by
ADR-0002; extraction only via the §4.2 rule (stable seam + module
outgrown + RFC + no facade change).
(d) Global-state shortcuts (ambient terminal mode, global theme, global
clock) that break headless testability — refused by ARCHITECTURE.md §2.3/§2.5.
(e) Async-runtime capture of the core loop — refused (Q9).
Mitigation for all five: the conformance checklist (ARCHITECTURE.md §11)
and the "expensive to change" list (Q14 / shared question J below).

**Q7 — Advanced capabilities: what exists as scope even if minimal at alpha?**
Focus model (at least single focus path with queryable ownership and
documented bubbling/focus order; focus scopes are a postponed refinement —
RFC-2 detail, and RFC-2 open Q5 agrees: single global focus path proposed,
scoped stacks deferred to the implementation RFC with a modal-dialog test —
no scope conflict, see record §R2-C2); scroll (at least vertical scroll offsets in list/table with
keyboard; smooth/pixel-style scrolling is out — cells only); forms
(input + focus traversal + validation hooks at alpha-minimal; full
form-builder is postponed); tables (headers, column constraints, selection;
virtualised million-row tables are postponed); state (ephemeral-vs-app
split per API_DESIGN.md §4; persistence is app-owned); animations (motion
must degrade to static; no animation is required at alpha, but the frame
model must not preclude a later time-scoped transform — `jilva-motion`
seam); async (opt-in bridge above the sync core only, runtime-agnostic;
see Q9); devtools (read-only observer seam; no alpha tooling required);
scaffolding (documented patterns at alpha; `jilva-cli` crate postponed).

**Q8 — Cross-language consistency: what travels to future siblings?**
Per ADR-0001, siblings (`jilva-go`, `jilva-ts`) are independent native
implementations, not FFI/shared-core. The portable artefacts are:
design principles (ARCHITECTURE.md §2), semantic theme-token vocabulary
(API_DESIGN.md §7), event-taxonomy shape, and docs/RFC culture. Nothing in
`jilva-rs` may be shaped "for future parity" unless Rust itself needs it.
Cross-language work starts only after the Rust alpha is genuinely usable
(ROADMAP Stage 3 / Phase 7). Conformance later is by specification
discipline (shared token lists, conformance-style examples), not code
sharing. No Rust API in this RFC constrains a sibling's idioms. Stated as
the explicit non-goal feasibility §6-Q6 asks for: theme-token and
event-taxonomy *shapes* are the portable artefacts future siblings align
to; nothing in the Rust API — signatures, traits, error types, module
paths — may be held back or shaped for portability. If a Rust-idiomatic
choice complicates a future sibling, Rust wins and the sibling adapts.

**Q9 — Initial refusals: what will we say no to first?**
Permanent (require a superseding ADR + RFC to revisit): GUI toolkit
ambitions (terminal only); full application framework (config, networking,
logging frameworks, DI containers); mandating or bundling an async runtime
(Tokio/async-std/smol) in the core contract; raw-ANSI styling in component
APIs (tokens only); ambient global state; backend-aware core types;
CSS-unit leakage into the public layout API; macro-primary component APIs
— a macro may only be additive sugar over a normative macro-free
definition, never the sole way to declare components (feasibility §4;
RFC-2 §7 takes the same stance: declarative helpers acceptable,
proc macros require a dedicated follow-up RFC); platform branching
(`cfg(windows)`) inside `jilva-core`; 3D rendering; sound/audio output;
direct framebuffer/GPU access. Postponed-but-not-refused items are in Q5 —
reviewers must not conflate the two lists.

**Q10 — Measurable performance goals (no invented benchmarks, no adjectives):**
Numbers belong to RFCs/benches, not to adjectives in docs
(ARCHITECTURE.md §8.11). This RFC sets the *measurable form* of the goals,
not the values: (i) full-screen diff within a documented frame budget at
three documented sizes (to be fixed by the rendering RFC; this RFC proposes
80×24, 120×40, 200×60 as the candidate sizes, not the budget); (ii) bounded
allocation in the hot path (rendering RFC states the allocation rule, e.g.
no per-frame heap growth for steady-state frames, with the metric and
harness named); (iii) no pathological quadratic behaviour on wide screens
(rendering RFC names the worst-case input class and the acceptance bound);
(iv) damage tracking composition: a one-cell state change in a large
static screen re-emits at most a documented bound of instructions
(rendering RFC fixes the bound). Posture is stated as *scaling behaviour*
(linear in cells, no worse) rather than frame-time numbers until benches
exist, and every golden-output test must be bench-reusable (deterministic
frames + fixed virtual sizes as harness inputs; feasibility §3). Until those RFCs land, **no latency,
throughput, or comparison claim** may appear in docs, examples, or commit
messages. This paragraph supersedes any informal "fast/blazing" language
anywhere it conflicts.

**Q11 — Testable "beautiful by default": what does the slogan mean operationally?**
"Beautiful" is not a reviewer feeling; it is four testable properties:
(a) zero-configuration output matches checked-in snapshots for every alpha
widget at each capability tier (truecolor/256/16 + no-unicode-fallback);
(b) spacing/alignment defaults (padding, gap, truncation with ellipsis,
double-width-safe wrapping) are specified per widget and snapshot-tested,
not inherited from ad-hoc app code; (c) degradation is specified: every
visual item's docs state how it renders at lower tiers (API_DESIGN.md §10),
and tier snapshots enforce it; (d) the curated default theme ships in the
default surface and custom themes override selectively with the default
filling gaps (no all-or-nothing authoring). A "taste" change that alters a
snapshot is a deliberate, reviewed diff — that is the whole mechanism.

**Q12 — Accessibility and degradation: what is guaranteed?**
(a) Capability ladder: truecolor → 256 → 16 → mono for colour (the mono
tier and `NO_COLOR`-maps-to-mono adopted from RFC-3 §5, Draft — see
Round-2 record; this extends the Q4 ladder, which previously stopped at
16); progressive key
encoding (legacy → Kitty-style where offered); mouse modes gated by
terminal support; alternate screen optional with inline fallback.
(b) No capability may be sniffed ad hoc mid-frame; profiles are detected at
startup, cached, and threaded explicitly (ARCHITECTURE.md §8.14).
Event-driven profile refresh at frame boundaries (RFC-3 open Q7 proposal)
is consistent with this rule — `NO_COLOR` / pipe-redirect changes travel
the theme/profile version-counter path, never interrupt a frame; the
lifecycle RFC must confirm the cadence.
(c) Behavioural guarantees: resize is always delivered; keyboard is always
available (at least legacy encoding); mouse absence never breaks keyboard
operation; colour absence never breaks information (status/ danger /
selected states must remain distinguishable by attributes/text, not hue
alone — RFC-3 details the mechanism).
(d) Documented non-goals for alpha: screen-reader/AT-SPI integration,
braille-table output, and high-contrast-forced themes are postponed (each
an RFC-sized topic); what alpha guarantees is *no information loss under
degradation*, not full assistive-technology support. Any claim beyond this
paragraph requires an accessibility RFC with testing methodology.

**Q13 — Change classes (shared question A): which exist as scope?**
Three change classes exist in scope, named so RFC-2/3 and the renderer can
reference them: **none** (no recomputation, no emission), **repaint**
(paint-only: style/text change within identical regions — must not
invalidate layout), **layout** (geometry-affecting: constraints, content
size, resize, or width-table-relevant text change — invalidates measure/
arrange for the bounded subtree, then repaint). **Reconstruct** (identity
change: component replaced, key changed, subtree rebuilt) is *not* a fourth
change class in this RFC's terms — it is the identity mechanism's output
(RFC-2), which *produces* layout+repaint work, not a separate invalidation
kind. Rationale: three classes keep the invalidation contract (§3.4/§3.6)
teachable and testable ("which changes force re-layout vs. re-paint only"
is already the ARCHITECTURE.md §3.6 question). **Round-2 status —
RESOLVED by adoption:** RFC-2 §5 (current) adopts this Q's taxonomy
verbatim — none / repaint / layout, no fourth class; mount/unmount,
key-set change, and type replacement are identity-mechanism outputs
producing layout + repaint work. One recorded deviation: RFC-2 makes
`none` a first-class return value rather than an implicit no-op —
within scope, flagged for reviewers in RFC-2 §5.

**Q14 — Smallest complete API and expensive-to-change choices (shared questions G + J):**
Smallest complete API (the G answer — the minimum surface that still
counts as the framework): construct (`App` builder: backend selection with
default, initial state, options) → run (loop owning setup/teardown,
returning a result with terminal restored) → describe (component/layout
combinators producing a pure frame description) → handle (event taxonomy
with total dispatch order + transactional update → re-render) → style
(semantic tokens + default theme + capability-automatic resolution) →
observe in tests (test double: scripted events in, instruction
stream/snapshots out). Every item must be reachable through the facade;
anything else is extension. Expensive-to-change choices (the J answer —
mistakes here force unlearning, so they need the most review): component
identity semantics; the event-taxonomy extensibility mechanism (how a new
variant avoids breaking exhaustive matches); the constraint vocabulary and
overflow/minimum-size policy; the token vocabulary (names are API);
capability-profile shape threaded into render/style; the facade error
taxonomy; backend seam interface (setup/teardown/read/write/size/
capabilities); tick/time-source abstraction. These seven are the
"measure twice" list for reviewers of RFC-2/3 and the engine RFCs.

### 2. Feature classification matrix

Legend: **Must** = alpha-blocking, in `0.1.0`. **Should** = designed-for
(seams/extension points present at alpha) but shipped post-alpha.
**Could** = future RFC with prototype evidence, no seam reserved.
**Out** = refused (revisit needs ADR + RFC).

| Area | Must (alpha) | Should (designed-for) | Could (future RFC) | Out of scope |
|---|---|---|---|---|
| Backend | Narrow seam (setup/teardown/read event/write instructions/size+capabilities); default backend (Crossterm-based per ADR-0003, version = impl detail); in-memory test double as public test API | Backend swap with no core/user-code change beyond construction edge | termwiz/second backend evaluation with prototype evidence | Hand-rolled console I/O owning all quirks; backend-aware core types |
| Lifecycle | setup → loop → teardown; restoration on normal, `?`-error, and panic (where feasible) paths; cooperative shutdown with result | Suspend/resume + shell-out hooks designed as edge, not core | Session persist/restore across processes | App recovery policy (app-owned) |
| Components | Component contract per API_DESIGN.md §4 (identity, state split, event consumption, composition, pure render); choice of flavour = RFC-2 | Keyed children, focus-order primitives stable for extension | Third-party widget ecosystem conventions | GUI widgets; DOM/web rendering targets |
| Layout | Flex row/column, gap, alignment, grow/shrink; grid tracks + placement; integer cell rectangles; documented remainder rule; graceful over-constraint behaviour (never panic/overlap silently); vocabulary closed at alpha — additions need an RFC (feasibility F1.1) | Spanning refinements; container-bounded invalidation cascades | Taffy-behind-the-seam adapter (ADR-0003 conditions) | CSS units/floats user-visible ("Taffy, all of it" without a cell-quantisation story); manual cell math required of apps |
| Styling | Semantic tokens only (no raw ANSI in component APIs) | Token extension mechanism | Per-widget visual redesign passes | Raw-ANSI styling escape hatches |
| Themes | Default curated theme in default surface; selective override with default filling gaps; automatic tier resolution | Theme packaging/sharing conventions | Theme gallery / external theme registry | All-or-nothing theme authoring |
| Keyboard | Legacy encoding baseline; progressive-enhancement ladder; documented degradation | Kitty-protocol progressive keys where offered, with push/pop enhancement flags paired to teardown on every exit path (lifecycle RFC owns the pairing; feasibility §2.4) | Key macro / remapping layer | Terminal key-protocol implementation inside core |
| Mouse | Click/scroll where supported, gated; absence never breaks keyboard operation | Drag + modes where supported | Gesture semantics | Mouse-required interaction design |
| Unicode | Grapheme-cluster segmentation with stated authority; width feeds layout as input, layout never shapes text; `Cell { symbol: char }` replacement scheduled no later than Rendering Engine (Phase 3) — the RFC first defining buffer/diff semantics specifies the grapheme-capable symbol type + width/segmentation authorities, before golden tests entrench a `char`-shaped API (feasibility §2.3, F1.2; authority candidates and blank/continuation semantics → Open Q13) | Width-table fallback policy when terminal disagrees | Custom shaping / complex-script shaping engine | Byte/`char`-count layout math |
| Truecolor | truecolor → 256 → 16 → mono automatic resolution; per-tier snapshots | Dithering/approximation policy detail (RFC-3) | — | Capability sniffing ad hoc mid-frame |
| Scroll | Vertical offsets in list/table + keyboard; truncation/ellipsis rules | Scrollable containers; horizontal scroll | Virtualised huge lists; smooth-scroll emulation | Pixel scrolling |
| Forms | Text input + focus traversal + validation hooks (minimal) | Form-builder helpers | Masked/complex editors | App-domain validation logic (app-owned) |
| Tables | Headers, column constraints, selection, double-width-safe truncation | Column pinning/sorting hooks | Virtualisation, cell spanning complexity | Spreadsheet engine |
| Focus | Single focus path, queryable ownership, total dispatch order (focused → ancestors → app fallback) | Focus scopes | Focus-memory/restore policies | Multi-cursor / multi-focus |
| State | Ephemeral-vs-app split with stated rule; transactional per-event update; intermediate states never painted | Selector/memo conventions | Time-travel debugging (devtools) | Persistence, networking, config (app-owned) |
| Animations | Frame model must admit later time-scoped transform; static output always valid | `jilva-motion` extraction when seam stable | Transition vocabulary RFC | Motion-required UI; sound-synced animation |
| Async | Sync core, zero async deps; documented user-side pattern for background work | Opt-in runtime-agnostic bridge above core (explicit scheduling) | Effect/timer first-class API (only via RFC) | Bundled/mandated runtime; executor types in public API |
| Devtools | Read-only observer seam (event log + frame stream, never hot path) | Inspector + golden-record tooling | Time-travel / replay UI | Profiling-in-production instrumentation |
| Scaffolding | Documented patterns + one-tiny-example-per-concept (facade-only) | `jilva-cli` as facade-only consumer | Project templates / generators | Package-manager / build-system tooling |
| Images/graphics | Specified fallback behaviour (text/half-block degradation rule stated, no silent gaps) | Seam reservation for protocol adapters | Sixel / Kitty / iTerm2-inline RFCs with prototype evidence | Native image decoding pipeline in core; GPU/framebuffer access |
| Sound | — | — | — | **Out.** No audio output, ever in current scope |
| 3D | — | — | — | **Out.** No 3D rendering, ever in current scope |
| Remote sessions | Correctness over SSH/multiplexers via degradation ladder (no special-casing) | Optimised redraw batching for high-latency links | Mosh/tmux-aware adaptations (evidence-gated) | Custom wire protocol; server-side rendering service |

Notes: (i) every Must row needs at least one headless test (golden-output,
scripted-event, or tier snapshot), and golden tests must be bench-reusable
(fixed sizes, deterministic frames — Q10); (ii) Should rows must not add public API
at alpha beyond the seam/extension point; (iii) Could rows must not be
cited as commitments in any other RFC.

### 3. Constraints restated (non-negotiable in this RFC)

MIT only (every crate, example, bench); Rust edition 2024; stable 1.98.1,
no nightly (MSRV number itself is a future RFC); zero-dependency
`jilva-core` within the workspace and platform-agnostic (no TTY, no
`cfg(windows)`); `jilva-terminal` the only crate touching terminal I/O or
naming a terminal library; facade `jilva` re-exports only; acyclic
dependencies (ADR-0002); workspace layout exactly per ARCHITECTURE.md §7;
no code changes accompany this RFC (prototype branches only, never
published, never substituting for the RFC).

### 4. Coherence inputs owned by this RFC for RFC-2 / RFC-3 / engine RFCs

- Shared **A** (change classes): none / repaint / layout exist; reconstruct
  is identity output, not a fourth class (§1 Q13). **Round-2 status:
  RESOLVED** — RFC-2 §5 adopts this taxonomy verbatim (record §R2-C1
  retired; `none` is a first-class return value there, flagged for
  reviewers).
- Shared **G** (smallest complete API): construct → run → describe →
  handle → style → observe-in-tests (§1 Q14). RFC-2/3 must show their
  additions keep this minimal path intact (progressive disclosure,
  API_DESIGN.md §1.3). Round-2: RFC-2 §7 (trait + builders + leaf
  closures) and RFC-3 §4 (delta themes) both preserve the minimal path —
  consistent. Gap: RFC-2 names no heterogeneous-children mechanism
  (record §R2-C3); if the mechanism adds public API, G needs a
  scope-RFC acknowledgement.
- Shared **J** (expensive-to-change): the seven-item "measure twice" list
  (§1 Q14). Engine and model RFCs must call out impacts on each item they
  touch. Round-2: RFC-2 identity/key semantics and RFC-3 closed-enum token
  set both land squarely on this list — Discussion must treat them as
  load-bearing, not sketch-grade.
- Shared **B** (theme propagation without rebuild): answered by RFC-3 §6
  (version counter + paint-only damage); this scope records the answer,
  with scoped-themes (RFC-3 open Q6) as a watch item (record §R2-C5).
- Shared **D** (capability entry): answered by RFC-3 §5 + shared-ownership
  note (detection in terminal, plain-data profile, pure core resolution);
  refresh cadence (RFC-3 open Q7) owned by the lifecycle RFC.
- Boundary consistency: §§3.1–3.10 ownership statements in ARCHITECTURE.md
  are assumed; where this RFC and ARCHITECTURE.md appear to disagree, the
  Open questions section governs — this RFC does not silently override.

### 5. ARCHITECTURE.md §8 consideration mapping (feasibility F1.5)

Every consideration has an owner as of Round 2. "Owned" means the named
RFC answers it; "scoped" means this RFC bounds it and a later RFC answers
it; "deferred" means explicitly postponed with the named condition.

| # | Consideration | Disposition |
|---|---|---|
| 8.1 | Cell buffering (ownership, bounds, empty-vs-styled-blank) | Scoped here (renderer owns buffer/diff model, §3.7); answered by the Rendering Engine RFC (Phase 3); blocked on the `char`-replacement scheduling in the Unicode matrix row |
| 8.2 | Differential rendering (granularity, cursor minimisation, worst-case cost) | Scoped here (Q10 measurable forms); answered by the Rendering Engine RFC |
| 8.3 | Unicode grapheme clusters (where segmentation happens, width authority) | Scoped here (Unicode matrix row); authorities named by the first buffer/diff RFC (Open Q12) |
| 8.4 | Double-width and emoji (width-table source, terminal-disagreement fallback) | Scoped here (Unicode/Truecolor rows); fallback policy = Should-level detail for the renderer RFC |
| 8.5 | Resize (observer, drop-vs-complete, re-layout ordering) | Scoped here (Q12c: resize always delivered); drop-vs-complete policy owned by the lifecycle RFC — RFC-2 §4 proposes "complete or drop, never half-painted" with the exact policy as tested implementation detail |
| 8.6 | Keyboard and mouse (encoding ladder, degradation) | Scoped here (Keyboard/Mouse matrix rows); ladder depth for `0.1.0` + Kitty teardown pairing owned by the lifecycle/event RFC (Open Q13) |
| 8.7 | Lifecycle and terminal restoration (all exit paths) | Scoped here (Lifecycle matrix row, Must); mechanism owned by the lifecycle RFC; RFC-2 §6 requires the guarantee hold for component code too |
| 8.8 | Component identity (mechanism + misuse behaviour) | Owned (proposed) by RFC-2 §2 (paths + keys + collision/mixing rules) — Draft, pending acceptance; storage backing → Open Q12 |
| 8.9 | Layout invalidation (dirty-propagation rule) | Scoped here (Q13 + Layout matrix row); taxonomy RESOLVED — RFC-2 §5 adopts none / repaint / layout verbatim; containers-bound-cascades detail owned by the layout RFC |
| 8.10 | Minimal redraw (damage → backend flow) | Scoped here (Q10-iv + Devtools seam); composition proof owned by the renderer RFC; RFC-2 debug damage-assertions are the widget-side backstop |
| 8.11 | Performance posture (numbers in RFCs/benches) | Scoped here (Q10 forms + scaling-behaviour rule); values owned by engine RFCs + benches |
| 8.12 | Optional async (opt-in bridge, runtime-agnostic) | Refused-in-core, scoped here (Q9 + Async matrix row); bridge design owned by a future lifecycle/effects RFC; RFC-2 `Effect`-as-data watched (record §R2-C4) |
| 8.13 | Testability without a terminal (golden output, scripted events, fixed clock) | Scoped here (Q3 test double, matrix note i, Q11 snapshots); fixed-clock/tick-source shape still open (Open Q3) |
| 8.14 | Cross-platform capability detection (profile cached, threaded explicitly) | Scoped here (Q12a–b); answered in proposal form by RFC-3 §5 + shared-D note; refresh cadence owned by the lifecycle RFC (RFC-3 open Q7) |

### 6. Hello-world brevity bar (feasibility F1.4)

Scope constraint, not taste: API_DESIGN.md principles 1 (no manual cell
math) and 3a (build-and-run helpers) are testable only against concrete
line counts. Reference figures from the feasibility report §4 (their
measurements, not ours): Ratatui hello-world ~10 lines; Cursive ~8 lines;
iocraft `element!` hello ~10 lines; Textual clock ~25 lines with CSS;
Bubble Tea shopping-list tutorial ~60 lines for Init/Update/View + `main`.
The bar for this framework: the layer-(a) hello-world stays at parity
(~8–15 lines, facade only, zero configuration), and the counter + list
examples in RFC-2 read at comparable brevity to the cited prior art. The
Q14-G sketch in §4 is ~14 lines excluding blank/comment lines and meets
the bar in proposal form. Discussion reviewers are asked to count lines on
every doc-example addition that claims layer-(a) status; any layer-(a) path
exceeding ~25 lines is a scope failure, not a docs failure.

```rust
// sketch — not final, not compiling
// The smallest complete API (shared question G), as a user would write it:
use jilva::{App, events::Event, theme::Tokens};

fn main() -> jilva::Result<()> {
    let app = App::builder()
        .with_default_theme()      // tokens only; no ANSI in app code
        .with_initial_state(my_state)
        .build()?;                 // construct without running (tests do this too)
    app.run(|state, event| {       // run owns setup/teardown + restoration
        match event {
            Event::KeyPressed(k) => update(state, k),  // one event → update → re-render
            Event::Resized(size) => relayout(state, size),
            _ => {}
        }
    })
}
```

## Prior art

Researched October 2026 from project docs/sources (verified facts; readings
marked interpretation). No superiority claims; each entry states what we
borrow and what we deliberately reject.

- **Ratatui** (Rust, MIT; forked from `tui-rs` 2023; docs.rs `ratatui`,
  ratatui.rs/concepts/rendering). Verified: **immediate-mode** rendering —
  every frame redraws the UI anew via `terminal.draw(|frame| …)`;
  intermediate-buffer + diffing model; `Backend` trait abstraction with
  backend crates (`ratatui-crossterm`, `ratatui-termion`, `ratatui-termina`,
  `ratatui-termwiz`); split crates (`ratatui-core` traits/types,
  `ratatui-widgets` built-ins); layout via `Constraint`/`Direction`/`Layout`;
  rich widget set (charts, sparklines, tables, gauges, lists). Borrow:
  backend-trait swappability, buffer+diff discipline, constraint vocabulary
  inspiration. Reject: leaving the run loop and state ownership to user code
  — Jilva owns the loop (Q1). Precision note (Phase 1.6): modern Ratatui
  ships `run()`/`init()`/`restore()` helpers with panic hooks, so the
  contrast is on loop+state ownership, not on restoration plumbing — which
  Ratatui also handles. Interpretation:
  Ratatui's positioning as renderer+widgets (user owns the loop) is the
  clearest contrast justifying our framework claim.
- **Bubble Tea** (Go, Charm; MIT — interpretation, widely reported; verify
  at implementation time). Verified: **Elm architecture**
  (`Init`/`Update`/`View`; `Update(Msg) → (Model, Cmd)`; `tea.Msg` is
  `interface{}`/any); `View() string` declares the whole UI per state;
  ecosystem split (Bubbles components, Lip Gloss styling). Borrow:
  message → state → render unidirectionality; total dispatch thinking;
  ecosystem separation (core vs. components vs. styling). Reject: stringly
  whole-screen view returns as the primitive (our renderer consumes
  regions + styled spans with damage tracking, §3.7); Go-idiom API shapes.
- **Textual** (Python, Textualize; MIT — interpretation; verify at
  implementation time). Verified: widgets + containers + `compose()` yields;
  **TCSS/CSS-driven** styling and layout (`CSS_PATH`, grid/dock, reactive
  queries); message-passing + `on_*` handlers; **async-first** (`@work`
  workers over async/threads); dev tooling (`textual-dev`, live CSS reload,
  `textual run`); web-developer mental model (LWN 2023, Real Python course).
  Borrow: declarative compose, reactive tokens-as-idea (ours are Rust tokens,
  not CSS), devtools-as-observer precedent, docs craft bar. Reject: CSS as
  the styling language (ours: semantic tokens, never stylesheets);
  async-coupled core (ours: sync baseline, opt-in bridge only).
- **iocraft** (Rust; MIT/Apache-style dual — interpretation; verify before
  citing in code RFCs). Verified via docs.rs/GitHub: `element!` macro with
  React/SwiftUI-like declaration; `#[component]` custom components;
  **hooks** (`use_state`, `use_future`); `TerminalBackend` trait seam;
  components module (`View`, `Text`, `TextInput`); re-exports `taffy`;
  inspired-by Dioxus and Ink (self-declared). Borrow: hooks-adjacent
  ergonomics as one RFC-2 candidate flavour; backend-trait seam precedent.
  Round-2 outcome: RFC-2 (Draft) decided trait + builders, declarative
  macros as additive sugar only, proc macros rejected without a dedicated
  follow-up RFC — consistent with this scope's Q9 macro rule.
  Reject (upheld): `taffy` re-export leaking engine types toward users
  (ADR-0003 forbids exactly this).
- **Cursive** (Rust; MIT-licensed crate, crossterm default with other
  backends available — verified via crates.io/docs.rs). Verified:
  **retained-mode** views (`views::Dialog`, `TextView`, `EditView`,
  `ListView`), `Cursive` root object, trait-based sizing/naming. Borrow:
  retained-mode ergonomics as an RFC-2 candidate; backend-switchability
  precedent. Reject: framework Erzählen that hides the loop's restoration
  contract — ours is explicit and tested per exit path.
- **Ink** (JS/TS, vadimdemedes; MIT — interpretation). Verified via README:
  **React renderer** (`react-reconciler`) for CLIs; **Yoga Flexbox** layout
  (every element flex-enabled; `Box`/`Text` host components); `Static` for
  big-output efficiency; component testing story. Borrow: flex-first layout
  vocabulary; `Static`-like big-output thinking for later (postponed
  optimisation, not alpha API). Reject: React-reconciler coupling and
  Yoga-types-visible API (same leakage objection as Taffy); JS-idiom shapes.
- **OpenTUI** (Zig core + TS bindings via Bun FFI; sponsors/Anomaly+OpenCode
  lineage; Yoga Flexbox; React/Solid/vanilla-TS bindings; in-memory testing
  rendering for assertions/snapshots; powers OpenCode in production —
  verified via opentui.com/docs, 2026 tutorials). Borrow: native-core
  performance seriousness (but our perf claims stay in benches, never prose);
  Flexbox-via-Yoga-behind-a-seam (consistent with ADR-0003's adapter rule);
  in-memory testing as first-class API (already our §3.5/§8.13 stance).
  Reject: Zig+FFI distribution model (ours: pure-Rust workspace, MIT-only,
  zero-dep core); multi-runtime binding
  complexity pre-alpha (ADR-0001 sequences siblings after Rust alpha).

## Non-goals

This RFC does **not**: choose the component-contract flavour (RFC-2);
define the theme schema, token list, or palette values (RFC-3); set the
MSRV number, lint set, backend dependency version, or CI shape; specify any
algorithm, data structure, or code; promise any postponed (Q5) item;
authorise any new crate (extraction rule, ARCHITECTURE.md §4.2, governs);
change any ARCHITECTURE.md boundary (conflicts → Open Questions).

## Alternatives considered

1. **Ratatui-style scope (renderer + widgets, user owns the loop).**
   Serious and respectable: smallest API, maximal user control, proven by
   Ratatui's adoption. Trade-off: restoration, teardown-on-panic, dispatch
   order, and degradation become per-app folklore — exactly the
   correctness-underneath properties Jilva claims as features. Rejected
   because "beautiful by default *and* correct underneath" cannot be
   delivered by a library that never sees the loop.
2. **Textual-style scope (CSS + async core + devtools in v1).**
   Serious: CSS unlocks designer participation; async-first matches modern
   workloads; devtools sell the framework. Trade-off: CSS commits us to a
   second language and its cascade bugs in a cell medium; async core forces
   a runtime choice on every user, contradicting ARCHITECTURE.md §2.4 and
   API_DESIGN.md §1.6. Rejected for alpha; revisited only as separate
   post-alpha RFCs (stylesheet-companion, runtime-agnostic bridge).
3. **Single-crate scope (everything in `jilva`, modules only).**
   Rejected by ADR-0002 (no stable one-dependency surface independent of
   topology; platform code shares a compilation unit with pure
   foundations). Recorded here because scope reviewers keep proposing it;
   the answer stays no without a superseding ADR.

## Open questions

1. Fractional-remainder distribution rule (floor + largest-remainder?
   deterministic tie-break?) — layout RFC detail, but scope needs the
   tie-break named so snapshots are reproducible. Consistent with
   ARCHITECTURE.md §3.6? (No known conflict.)
2. Minimum terminal size policy: below what size do we degrade (scroll?
   truncate? refuse with a clean error + restored terminal)? Needs RFC-2 +
   layout RFC agreement.
3. Tick/time-source abstraction: fixed clock trait vs. injected duration —
   API_DESIGN.md §11.3 leaves open; scope needs it named because motion,
   tests (fixed clock), and the fixed-clock testability rule (§2.5) all
   depend on it. Flagging as scope-affecting, not decided here.
4. Event-taxonomy extensibility mechanism (non-exhaustive markers vs.
   wrapper enums) — RFC-2 detail, but listed in the J "measure twice" set
   because it breaks exhaustive matches. No ARCHITECTURE.md conflict.
5. Focus scopes in or out of alpha-minimal? This RFC scopes single-path
   focus as Must and scopes as Should; RFC-2 open Q5 takes the same
   position (single path proposed, scoped stacks deferred to the
   implementation RFC with a modal-dialog test). If that implementation
   RFC shows forms/dialogs require scopes at alpha, this RFC amends —
   no amendment needed while the positions agree.
6. Image/graphics fallback rule: is "state a degradation rule with no
   silent gaps" sufficient scope for alpha, or does even the rule need a
   dedicated RFC first? Provisional answer: the rule lives in the backend
   RFC; protocols themselves are Could.
7. Potential ARCHITECTURE.md tension (flagged, not overridden): §4.1 says
   `jilva-core` contains "facade error taxonomy (as internal types
   re-exported via facade)" while §3.9 says "only the facade error is
   public API" — consistent on a careful read (internal types, public
   re-export), but RFC reviewers should confirm the intended visibility
   wording so a future crate split (`jilva-error`?) is not accidentally
   precluded. No change proposed here.
8. Accessibility testing methodology for the "no information loss under
   degradation" guarantee (Q12c): RFC-3 §"Testing strategy" (Draft) now
   proposes the harness — tier snapshot matrix, mono distinguishability
   assertions, contrast checks, `NO_COLOR` and unknown-background tests —
   which this scope accepts as the proposed answer. Residual scope
   question: where those snapshots live (facade test surface vs. core
   module tests) and who runs the custom-theme check helper in CI.
9. Perf-candidate sizes (80×24, 120×40, 200×60) and the four measurable
   forms in Q10: confirm or replace in the rendering RFC; this RFC fixes
   only the form, never values.
10. Heterogeneous children (RFC-2 gap, feasibility F2.2): RFC-2 sketches
    `frame.column([Header, TodoList, StatusBar])` — mixed types in one
    child list — without naming the mechanism (dyn-compatible trait
    object? type-erased element? enum dispatch?) or its allocation
    behaviour (Box per child? Rc? arena?). "Composition is first-class"
    is unimplementable without this paragraph. Owned by RFC-2 (or its
    implementation RFC); if the mechanism adds public API (e.g. an
    erasure type, macro-generated glue), shared-G acknowledgement here
    is required.
11. Keyed-identity storage backing (feasibility risk #3): RFC-2 §2–§3 name
    a "framework slot map keyed by component identity" with misuse
    behaviour, but not the backing tech (SlotMap / generational handles /
    Pin / plain map). Minor: deferral to the implementation RFC is
    acceptable provided no storage type leaks into public API and
    collision/use-after-reorder behaviour stays as RFC-2 specifies.
12. `Cell`-replacement details (feasibility §2.3–§2.4, §6-Q2/Q3): the
    Unicode matrix row fixes scheduling (no later than Phase 3); still
    owned by the first buffer/diff RFC — symbol-type shape
    (String-family/SSO, MIT-gate justification per §6.6), width authority
    (`unicode-width`, `width` vs `width_cjk`), segmentation authority
    (`unicode-segmentation`), blank/continuation equality semantics, and
    the truncation rule (never split a grapheme or wide cell; 1-column-
    remainder edge behaviour). Deferral past Phase 3 is challenged:
    golden tests written against `char` cells would need rewriting.
13. Keyboard ladder depth for `0.1.0` (feasibility §6-Q4): legacy-only vs.
    Kitty push/pop included. Owned by the lifecycle/event RFC; scope
    requires the documented ladder either way, plus teardown pairing for
    any enhancement flags pushed (matrix Keyboard row). Cursor single-
    owner designation rides with the same RFC (feasibility §2.4).
14. RFC-2 `Effect` vocabulary vs. Async scope row (watch, not breach):
    the Must cell says "documented user-side pattern", Should the opt-in
    bridge, Could a first-class effect/timer API. RFC-2 proposes
    data-only `Effect` (`After`, `Task`) executed by the bridge with zero
    async deps in core — read here as the Should-level bridge taking
    shape early, acceptable iff core ships no executor types and no
    async dependencies (RFC-2 §7 asserts both). If `Effect` grows
    timers/threads inside core, scope amendment is required.
15. `Outcome::Observed` (RFC-2 §4, flagged by its own author as the only
    non-minimal piece): no scope objection in principle, but shared-G
    minimalism applies — if it survives to implementation, the layer-(a)
    path must not require understanding it.
16. Theming watch items (no action while positions agree): RFC-3 ships
    two built-ins (`jilva-dark` default + `jilva-light`) where this
    scope's matrix says "default curated theme" — read as compatible
    (one default surface, two values; snapshot matrix stays bounded). A
    third built-in (RFC-3 open Q9, high-contrast) or subtree-scoped
    themes (RFC-3 open Q6) would each need a scope check at proposal
    time. Token renames/additions post-acceptance require an RFC
    amendment (Q3 rule + J list); RFC-3's closed-enum breaking-change
    acknowledgement is consistent with that rule.

## Drawbacks

Honest costs: (i) owning the loop makes Jilva harder to adopt incrementally
inside Ratatui/Cursive apps — framework commitment vs. library
drop-in; (ii) tokens-only styling frustrates power users who want raw
escape control — the cost of testable degradation; (iii) sync-core +
runtime-agnosticism pushes async complexity onto users until the bridge RFC
lands; (iv) the Must list (§2) is large for an alpha and risks schedule
slip — the mitigation is postponing Should/Could aggressively, not
promoting them under pressure; (v) documentation burden: every public item
needs runnable facade-only doc examples plus per-tier degradation notes
(API_DESIGN.md §10) — real authoring cost, accepted as the price of the
"docs are part of the API" principle.

## Round-2 cross-review record (Draft-stage appendix, 2026-10-16)

Reviewed: RFC-2 (component model, Draft), RFC-3 (theming, Draft),
`docs/research/ARCHITECTURE_FEASIBILITY.md` Round 1 (findings F1.1–F1.5,
top risks, §6 questions). Method: each sibling checked against this
scope's matrix rows, Q1–Q14, and J-list; each feasibility finding accepted,
rebutted, or converted to an Open Question. Nothing in this appendix amends
any RFC — all three stay Draft, and contested items stay contested.

### Scope conflicts and gaps

- **R2-C1 — RESOLVED by adoption: change-class taxonomy (shared A).**
  This RFC (Q13): none / repaint / layout; reconstruct is identity
  output. RFC-2 §5 (current) adopts this taxonomy verbatim, with `none`
  as a first-class return value (flagged for reviewers there). The
  earlier paint / layout / structure triple is withdrawn; no joint
  amendment outstanding on this item.
- **R2-C2 — AGREED: focus scopes deferred.** Both RFCs scope single-path
  focus as Must, scopes as Should/deferred (this Q7, RFC-2 open Q5).
  Reopening trigger defined in Open Q5.
- **R2-C3 — GAP in RFC-2: heterogeneous-children mechanism unnamed**
  (feasibility F2.2). Tracked in Open Q10; shared-G acknowledgement
  required if the mechanism adds public API.
- **R2-C4 — WATCH: `Effect` vocabulary vs. Async scope row.** Read as the
  Should-level bridge forming early; conditions and amendment trigger in
  Open Q14.
- **R2-C5 — AGREED (with standing triggers): theming coherence.** RFC-3
  answers shared B (version + paint-only damage) and D (profile-as-data,
  pure core resolution, backend never sees tokens — upholding ADR-0003
  decision 2); mono tier adopted into Q4/Q12; two built-ins read as
  compatible. Standing triggers (renames, third built-in, scoped themes)
  in Open Q16. RFC-3's Q12c harness proposal accepted as the answer,
  residue in Open Q8.
- **R2-C6 — MINOR: keyed-storage backing deferred** (feasibility risk #3).
  Acceptable under the conditions in Open Q11.
- No ARCHITECTURE.md boundary violations found in either sibling: RFC-2
  stays inside §§3.3–3.4 (its open Q6 so states), RFC-3 inside §3.8
  (its alternative 3 explicitly upholds ADR-0003). The §3.9/§4.1
  error-visibility wording flag (Open Q7) is unaffected by either draft.

### Feasibility dispositions (F1.1–F1.5 + risks)

- **F1.1 (bound constraint vocabulary) — ACCEPT, incorporated:** closed-
  at-alpha vocabulary + Taffy cell-quantisation condition in the Layout
  matrix row.
- **F1.2 (`char`-cell replacement scheduling) — ACCEPT scheduling,
  CONVERT details:** no-later-than-Phase-3 rule in the Unicode matrix
  row; authority choices (width/segmentation source, `cjk` feature,
  SSO/symbol shape, blank/continuation semantics, truncation edge rule)
  converted to Open Q12, owned by the first buffer/diff RFC.
- **F1.3 (non-goals: async, CSS-units, macro-primary) — ACCEPT the gap,
  incorporated:** macro-primary refusal added to Q9 (first two were
  already present).
- **F1.4 (hello-world brevity with line counts) — ACCEPT, incorporated:**
  new Proposal §6 with the report's figures attributed and a ~8–15 /
  ~25-line scope bar.
- **F1.5 (§8 mapping) — ACCEPT, incorporated:** new Proposal §5 mapping
  all 14 considerations to owner/scope/defer dispositions.
- **Top risks:** #1 entrenchment → scheduling + Open Q12; #2
  heterogeneity → R2-C3/Open Q10; #3 key storage → Open Q11; #4 Kitty
  ladder → Keyboard row + Open Q13; #5 macro-primary → Q9.
- **Partial REBUTTAL (feasibility §3.2):** the report asks the RFC to
  "state which layer carries the optimisation" between invalidation and
  damage-tracking. Scope keeps *both* layers required (ARCHITECTURE.md
  §§8.9–8.10 demand both; dropping either would contradict the
  architecture). The renderer RFC owns the idle-cost proof
  ("O(1)-ish when idle" per layer), not a scope-level choice of one
  layer over the other.
- **Feasibility §6-Q5 (test-double timing model) — scope position
  recorded:** scripted event sequences are sufficient for the Phase 2
  exit criterion; an event-timing model (tick sources, coalesced
  resizes) is Could-level unless the lifecycle RFC evidences otherwise.
- **Feasibility §6-Q4/Q2 low-level choices** (ladder depth, `cjk`
  feature, SSO justification) are owned by lifecycle/renderer RFCs as
  recorded in Open Q12–Q13; scope does not pre-decide them.
