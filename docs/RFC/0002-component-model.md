# RFC-0002: Component Model, State Ownership, Event Flow

- Status: Draft
- Author(s): Subagent 2 (Component Model & Reactivity Architect, Jilva Phase 1.5)
- Created: 2026-10-16
- Related: docs/ARCHITECTURE.md (§3.3, §3.4, §8.8, §8.9), docs/API_DESIGN.md (§4, §5),
  docs/adr/0002-workspace-structure.md; RFC-1 (scope parent — change classes,
  focus, J-list), RFC-3 (theming — style/layout invalidation); Round-2 input:
  docs/research/ARCHITECTURE_FEASIBILITY.md (findings F2.1–F2.6)
- Amendment history: see bottom. Status remains Draft per docs/RFC/README.md.

## Summary

`jilva-rs` adopts a **hybrid component model**: an Elm/MVU-style unidirectional
loop (events → update → render) over app-owned state, with a lightweight
**retained identity layer** (stable paths + explicit keys) and a **pure,
immediate-mode-flavoured view function** that describes each frame from scratch.
Components are plain Rust values implementing a small `Component` trait (no proc
macros, no global registry, no async runtime types in signatures). Component-local
ephemeral state (cursor, scroll offset) lives in framework-owned state slots
keyed by identity; domain state stays in the app model; derived values are
computed in `view`, never stored. Events dispatch focused-component → ancestors
→ app fallback, with explicit consumption. Re-render is dirty-region driven
(full re-render is always correct; layout vs. paint-only invalidation is an
explicit change class). Everything runs headless against the test double.

> All code sketches below are **PROPOSED API — NOT YET IMPLEMENTED**.
> They do not compile and are not normative; only the contracts in §§1–7 are.

## Motivation

ARCHITECTURE.md §3.3 deliberately leaves the component flavour open but fixes
what the decision must specify: identity, state ownership, event handling,
composition. API_DESIGN.md §4 adds hard constraints: identity mechanism with
misuse behaviour, local-vs-app state rule, documented bubbling/focus order, and
pure render. Without this RFC, implementers would improvise each of these and
users would have to unlearn them later — exactly what the RFC process exists to
prevent. Widget authors (future `jilva-widgets`) additionally need to build
reusable stateful widgets (input, list) without access to app privates
(shared question F), and every boundary must be headless-testable (question E).

## Proposal

### 1. The loop (normative)

One event → one transactional update → at most one painted frame.
Intermediate states are never painted (API_DESIGN.md §5):

```text
event ──► route to focus path ──► update (Model + local slots, &mut) ──► view (pure, &)
              │                              │                              │
              │                              ▼                              ▼
              │                        change class                 frame description
              │                        (none / repaint / layout — §5)      (regions + spans)
              │                              │                              │
              └──── unconsumed ──► app fallback                              ▼
                                                              renderer diff + backend write
```

Rules:

1. `update` receives `&mut` access to exactly one owner: either the app model
   (app messages) or one component state slot (component messages), plus a
   bounded `Effect` queue (see §5). It never receives the terminal, the
   renderer, or sibling state.
2. `view` receives only shared references (`&Model`, `&Slot`) plus
   explicit `&Context` (theme tokens, focus path, capability profile, tick
   clock value). `view` receives no regions, rectangles, or cell coordinates;
   all geometry is framework-produced (§8 pipeline rules 3–4). It performs no I/O, no interior mutation of framework state, no
   global access. Same inputs ⇒ same frame description (deterministic, question C).
3. No reentrancy: `update` cannot synchronously dispatch a new event. Anything
   that would recurse (e.g. "selecting this item opens a dialog") is enqueued
   as a message/effect and processed after the current update completes, in
   FIFO order. This makes event ordering total and replayable.
4. Batching: all messages enqueued during one update drain before the next
   `view`. Exactly one `view` + diff per source event (plus at most one extra
   frame if effects enqueue follow-ups, bounded by a documented cap after which
   the loop reports `Error::UpdateOverflow` instead of hanging).

### 2. Identity (normative)

Two complementary mechanisms; both are stable across frames:

- **Positional path.** Each mounted component has a path (e.g.
  `root / 0 / 2` = third child of first child of root). Paths are assigned
  deterministically by a depth-first walk of the tree the `view` describes.
  Positional identity is the default and suffices for static structure.
- **Explicit keys.** Any child in a dynamic sequence carries a
  user-supplied `Key` (an opaque, hashable, `Clone + Eq + Hash` value; typically a
  domain id) — **required by contract for dynamic sequences, enforced at
  runtime, not by the type system** (evidence: feasibility exp03). When
  siblings carry keys, identity follows keys, not positions. Static
  structure needs no keys.

Contract:

| Situation | Rule |
|---|---|
| Stable structure, no keys | Positional identity; state slots preserved |
| Keyed siblings reorder | State follows the key (scroll/cursor stay with the item) |
| Key appears / disappears | Slot created on first mount (`init`), dropped on unmount with `cleanup` called (§6) |
| Unkeyed dynamic sequence (keys omitted where required) | Framework flags it (debug assertion + documented runtime outcome flag); falls back to positional with the fallback recorded — never silent |
| Keyed ↔ unkeyed mixing within one sibling list | Forbidden: framework logs/detects in debug builds and falls back to positional with a documented warning; never silent state corruption |
| Key collision (duplicate keys among siblings) | First wins; extras get fresh slots each frame (i.e. stateless) + debug assertion + collision count. Never panic in release |
| Stale identity handle (slot dropped since) | Resolves to `None`; holders must handle absence — no dangling access |
| Component type at an identity changes (replacement) | Old slot dropped (with `cleanup`), new slot initialised. Type change ⇒ new component, even if key/position match |
| Conditional branch (`if/else` swapping subtrees) | Positional identity applies: same position + same type ⇒ same slot. Users who need branch-independent state use keys or lift state to the model |

This answers ARCHITECTURE.md consideration §8.8 directly and gives the misuse
behaviour API_DESIGN.md §4 demands.

### 3. State ownership (normative)

Four classes, exactly one owner each (shared question A territory — the change
classes in §5 build on this):

1. **App (domain) state.** Owned by the user struct (`Model`). Business data,
   selection-as-domain-fact, fetched content. Only app `update` mutates it.
2. **Component-local (ephemeral) state.** Owned by the framework slot map,
   keyed by component identity (§2): cursor offset, scroll position, draft text,
   open/closed. Only that component's `update` mutates its slot, via `&mut`.
   Slots are created by `init`, destroyed on unmount (`cleanup` runs).
   Components never own `Rc<RefCell<…>>` graphs to framework state; interior
   mutability inside a slot is allowed only for memoised derived caches that
   are observably pure (see below).
3. **Transient (per-frame) state.** Owned by the `view` call: hover/pressed
   derived from last event + focus path, animation phase from the tick clock.
   Recomputed every frame, never stored in slots. Storing transient values in
   slots is a documented anti-pattern (stale hover after resize).
4. **Derived state.** Computed in `view` from (1)+(2), never stored anywhere
   except behind a purity-preserving memo (`OnceCell`-style cache keyed on
   inputs is permissible inside render helpers because it is unobservable).
   Filtered/sorted lists, visible-window slices, resolved style tokens.

Communication rule (no hidden globals, no sibling writes): components emit
**messages upward**; parents decide. A child never mutates parent or sibling
state — it returns `Outcome::Emit(msg)` and the loop routes it. Shared
(domain) data needed by many widgets (e.g. theme, focus path, capability
profile) is passed down as explicit `&Context` parameters (environment),
threaded by the framework, never a thread-local or singleton.

Ownership mechanics in Rust terms:

- `Model` and each state slot are plain owned values (`S: Sized`, no required
  heap allocation; **no `Send`/`Sync` bounds on models or slots** — threading
  policy (b): `Send` lives only on bridge job/completion types at the async
  boundary, so single-threaded apps keep `Rc`/`RefCell`/`Cell` ergonomics;
  see §7 and open-question resolution note below).
- `update: &mut S` — exclusive access is structural, not via `RefCell`; the
  borrow checker enforces "one owner mutates" at compile time.
- `view: &S` — shared borrows compose freely (parent views child state through
  `&`), so deep trees never fight the borrow checker.
- No lifetimes escape: frame descriptions are owned values (regions + styled
  spans as data), never `&` into component state, so the renderer may retain
  the previous frame without borrowing the tree. Partial-output rule: a
  failed `view` yields no description — there is no partially-built frame in
  user-visible state (evidence: resolution B1/B2 measurements).

### 4. Event propagation (normative)

Event taxonomy (extends API_DESIGN.md §5): `Key`, `Mouse` (click/drag/scroll,
gated by capability), `Resize(w,h)`, `Tick(now)`, `Message(M)` (user channel),
`FocusGained/FocusLost` (synthesised by the framework, not the backend).

**Extensibility mechanism (J-list item; RFC-1 OQ4 deferred here).** The
taxonomy is a `#[non_exhaustive]` enum for framework events, so adding a
variant never breaks exhaustive matches in user code; app-defined events
travel exclusively through the `Message(M)` wrapper with a user-chosen `M`
(no framework matching over user types). New framework variants are
introduced as `Ignored`-by-default at every dispatch site, so old widgets
keep working. This is the whole mechanism — no parallel registry.

Dispatch order — total, exactly one path per event:

1. **Resize** broadcasts: every mounted component receives it top-down
   (parents before children) because regions change meaning; then re-layout,
   then re-render. In-flight frames complete or are dropped — never half
   painted (ARCHITECTURE §8.5; exact drop-vs-complete policy is an
   implementation detail with a test).
2. **Key / Mouse** go to the **focus path** leaf first, then bubble through
   ancestors in order, then to the **app fallback**. First consumer wins:
   a component returns `Outcome::Consumed`; otherwise `Outcome::Ignored` and
   the loop continues up. Mouse events hit-test against layout regions first;
   a click outside every component goes straight to app fallback.
3. **Tick** goes to components that subscribed (subscription is declared state,
   e.g. "spinner active"), then app fallback. Unsubscribed components are never
   woken — this is what keeps idle apps at zero CPU. The `now` value is an
   injected plain-data timestamp (position on RFC-1 OQ3: injected value, not a
   clock trait in component signatures; the lifecycle RFC names the source and
   the fixed-clock test seam — components stay clock-agnostic and headless).
4. **Message** (child-emitted or effect-produced) routes to the declaring
   owner: component messages to the emitting component's slot, app messages to
   app `update`.

Consumption rules: consuming an event stops propagation (no "peek and pass"
except via explicit `Outcome::Observed`, which records e.g. analytics without
claiming the event — reviewers should scrutinise whether `Observed` survives
to implementation; it is the only non-minimal piece). `FocusGained/FocusLost`
are delivered regardless of consumption (they describe routing, not input).
Focus ownership is always queryable (`app.focus_path()`), and focus moves only
through declared outcomes (`Outcome::RequestFocus(id)`), never by components
grabbing a global. **Scoping resolved per RFC-1:** alpha ships the single
focus path only (RFC-1 Q7/Q14 Must); focus scopes (modal/dialog trapping) are
a postponed Should-refinement, not a mandatory alpha mechanism. A dialog that
needs trapping at alpha does it as app-level fallback logic, explicitly
documented as a stopgap until the scopes RFC lands.

Reentrancy and ordering: handlers return data (`Outcome`), never invoke the
loop. All cross-component effects are FIFO messages. Given the same initial
state + same scripted event sequence, the frame stream is bit-identical
(deterministic updates, question C; the headless test double asserts this).

### 5. Render contracts — what recomputes when (normative, question A)

Adopts RFC-1 Q13 verbatim: three change classes named **`none` / `repaint` /
`layout`**. There is deliberately **no fourth class**: mount/unmount,
key-set change, and type replacement ("reconstruct") are outputs of the
identity mechanism (§2 + §8), and they *produce* `layout` + `repaint` work —
they are not a separate invalidation kind. If implementation shows a case
needing a fourth class, this RFC amends RFC-1 explicitly (per its Q13 rule)
rather than drifting.

- **`none`:** no recomputation, no emission. Returned when an update changed
  nothing observable (e.g. keypress with no binding). The loop skips `view`
  for that subtree entirely; with `none` across a whole batch, no frame is
  produced. (Deviation note: RFC-1 lists `none` as existing scope; this
  paragraph makes it a first-class return value rather than an implicit
  "no-op" — strictly within scope, flagged here for reviewers.)
   In Rust these surface as `Change::None`, `Change::Repaint`,
   `Change::Layout` (lowercase names are the contract concepts; PascalCase
   names are their enum variants).
- **`repaint` (paint-only):** text/attribute/cursor change within identical
  regions — must not invalidate layout (RFC-1 Q13 wording). Invalidates the
  component's damage rectangle only; no re-layout, no parent involvement.
  Pipeline gating: `repaint` reuses the previous layout (skips the layout
  stage of the §8 pipeline); `none` skips describe/reconcile/layout/paint
  entirely. **Theme, token, variant, and
  capability-profile changes are always `repaint`** (RFC-3 §6 R5): style
  resolution output carries no geometry by construction, so a theme switch
  provably cannot move anything. Component-side
  plumbing for RFC-3 shared question B: the loop holds `theme_version` /
  `profile_version` counters; a version mismatch on an otherwise clean tree
  re-resolves attributes and repaints with measure/arrange skipped.
- **`layout` (geometry-affecting):** constraint changes, child-set changes,
  resize, and **width-table-relevant text change** (RFC-1 Q13 wording —
  content whose measured size changed re-measures; same-size text stays
  `repaint`). Invalidates the component's subtree layout; propagation stops
  at the nearest ancestor whose region is unchanged (containers bound
  cascades — ARCHITECTURE §8.9).

Each `update` returns its change class; the loop takes the maximum over a
batch (`none < repaint < layout`). `update` reports both axes: consumption
(`Outcome`) and invalidation (`Change`). The exact carrier (whether `update`
returns a `Change`, an `(Outcome, Change)` pair, or `Outcome` carrying a
`Change` field) is fixed by the implementation RFC, but both values are
required outputs of every `update` — a sketch returning only one axis is
illustrative shorthand, not the signature. Over-reporting (claiming `layout` when
`repaint` sufficed) is always correct, only slower — so widgets default to
the safe class and opt into precision. Under-reporting is a bug class the
test double catches: debug builds diff the predicted damage against actual
frame differences and assert containment ("avoidance of wasted work" is
thus tested, not hoped for).

Headless testability (question E): `view` is callable without a terminal,
backend, or clock (clock injected as a parameter). Golden tests assert
frame-description snapshots; event-sequence tests replay scripts through the
loop with the in-memory backend and assert the frame stream. Every contract
in §§1–4 has at least one such test. Version-driven work (theme/profile
bumps, §5 `repaint`-only) joins the batch maximum as an implicit `repaint`
vote: it can lift a batch from `none` to `repaint` but never to `layout`
(otherwise an idle tree could skip a theme-driven frame entirely).

### 6. Lifecycle (normative)

`init → mount → (update → view)* → unmount → cleanup`, with errors at any
`update` unwinding through lifecycle teardown (ARCHITECTURE §3.9: terminal
always restored; the app decides recovery policy from the returned error).

- `init() -> Result<S, Error>`: construct default slot state. Pure.
  Fallibility covers allocation failure and bad config — errors surface as
  facade errors with component-path context.
- `mount`: framework assigns identity, inserts slot, delivers current context.
  No user code runs at mount except `init`.
- `update`: as §1. May return `Err`; the loop abandons the batch, tears down
  cleanly, reports.
- `unmount/cleanup(S)`: runs exactly once per mounted slot, including on error
  paths where feasible. Releases resources the slot owns (timers unsubscribed,
  temp files). Must not fail (signature returns `()`; log-only internally).
- User-callback panics (a widget `update` panicking) are a violated invariant
  only if the framework caused them; otherwise they unwind through teardown
  like errors (panic-guard mechanism is lifecycle-RFC detail, but this RFC
  requires the guarantee hold for component code too).

### 7. Ergonomics (normative direction)

- **Primary:** a `Component` trait with associated `State`, `Message` types
  plus builder-style constructors for configuration (`Input::new().prompt(…)`).
  Closures for leaf handlers (`on_select(|model, item| …)`) to keep simple
  apps to a few lines.
- **Declarative macros are acceptable** (`vec!`-style `column![…]` helpers) for
  tree construction; **proc macros are rejected without a dedicated follow-up
  RFC** showing the derive cannot be expressed as a trait + builder (constraint
  from task brief; rationale: proc macros cost compile time, obscure errors,
  and freeze syntax pre-`0.1.0`).
- **Macro-free normative core (F2.5 commitment).** Whatever sugar later
  exists, the normative definition of every component API is its macro-free
  expansion, presented in the implementation RFC with rustdoc-rendered output
  and a misuse-case compiler error. Macros are strictly additive: anything
  expressible with a macro is expressible without it, and tests exercise the
  macro-free path. A macro as the *primary* component API is rejected
  (feasibility §4: non-`dyn` traits force macro glue that hides the contract
  from rustdoc and errors — which is why §8 keeps the render seam
  `dyn`-compatible by construction).
- **Required-method count (F2.5).** Four required methods — `init`, `update`,
  `view`, `cleanup` — justified against Cursive's `View` (layout, draw,
  on_event, required_size, focus, …): each required method is a method every
  third-party widget implements forever, so the count stays at the minimum
  that satisfies §3.3 (identity/state/events/composition). Everything else
  (subscription flags, focusability, debug labels) is a defaulted method or
  builder option.
- **Third-party seam order (F2.5).** External widget authors depend on
  `jilva-core` while it churns pre-`0.1.0` (deliberate ADR-0002 trade);
  stabilisation order is component contract first, extraction later with a
  migration note — so the first external widget crate cannot freeze an
  immature seam by accident.
- **No async in signatures** (question H): `update` is synchronous `fn`.
  Effects (timers, background work) are declared as data
  (`Effect::After(dur, msg)`, `Effect::Task(fn_once_bridge)`) executed by an
  opt-in bridge layer above core; core itself has zero async dependencies and
  no executor types. Constraint: core names no thread, no executor, no spawn
  point — `Effect::Task` is opaque routed data, executed above core only
  (evidence: feasibility exp04). `Effect::Task`'s job closure and its
  completion value are `Send`-bounded at the bridge (policy (b), threading
  rule above); core routes the effect without executing, spawning, or
  naming threads.

Threading: single-threaded loop is the baseline. **No `Send`/`Sync` on models
or slots** (policy (b), evidence: E0277 against universal `Send` in
feasibility exp04): `Send` is required only on effect-bridge job and
completion types that cross threads; the bridge moves completions back as
messages. Multi-threaded rendering is a non-goal.

### 8. Rust feasibility end-state (normative; answers F2.1–F2.6)

This section converts the feasibility challenges into load-bearing contract.
Each item names the end state so implementation never re-lives the migration.

**F2.1 — Render-signature end state.** `update` takes `&mut` (exclusive,
structural — never `RefCell`, never a `&mut` root handle in Cursive style);
`view` takes `&` only. There is deliberately **no `&mut self` render**: any
cursor/scroll memoisation that needs mutation happens in `update`, whose
`&mut Slot` is the single legal mutation site (this is Ratatui's
`Widget`/`StatefulWidget` split carried to its conclusion: state and render
are separate types from the start, not after a breaking migration). `view`
returns nothing by reference: frame descriptions are **owned data**
(regions + styled spans), so no returned lifetime threads through the
event→update→render transaction (feasibility §1.1 challenge discharged —
neither whole-model clone nor arena is needed because nothing is borrowed
across the boundary). Concretely: **`&mut Frame` (or any mutable render
surface) MUST NOT appear in the `Component` trait.** The sketches' old
`frame: &mut Frame` parameter is superseded by the return-owned form below;
`Frame` names the framework-internal cell buffer only (pipeline rule 6).

**F2.2 — Heterogeneous children.** Named mechanism: a **`dyn`-compatible
render seam**. The view side of `Component` is object-safe (`&self` methods,
no generic methods, no `Sized` bounds on the render path), so mixed children
coexist as `Vec<Box<dyn Render>>`-shaped trait objects behind a small
framework enum for the common homogeneous fast path (static dispatch, no
boxing) with automatic fallback to boxing for mixed lists. Allocation rule:
at most one `Box` per heterogeneous child, allocated once at mount and reused
across frames (slots are stable — §2/F2.3), never per-frame boxing in
steady state; the homogeneous fast path allocates nothing. "Composition is
first-class" (API_DESIGN §4) is implementable from this paragraph alone.

**F2.3 — Identity storage.** Keys (§2) are half a design without storage;
here is the other half: framework-owned **generational slot table** —
`Vec`-backed, hand-rolled (zero-dep constraint forbids reaching for an
external `SlotMap`; no new dependency, no seam decision smuggled in).
Each mounted component holds a generational index; a sibling-level map from
`Key → index` (hash map with user-supplied keys, `Clone + Eq + Hash`)
survives across frames while positions shift. Stable addresses across frames
come from indirection through the table, never `Pin`. Collision/use-after-
reorder behaviour is §2's contract (first-wins + debug assertion; state
follows keys on reorder); the table makes that behaviour implementable.

**F2.4 — Borrow feasibility.** No stored borrows anywhere: parents view
children through `&`, updates hold exactly one `&mut` at a time, and the
slot table (not the tree) owns all mutable state — so deep trees never
double-borrow. No `unsafe` is required for any of this (compatible with a
`forbid(unsafe_code)` posture); the generational table is safe indexed
access. Threading policy (b): models and slots carry **no** `Send`/`Sync`
bounds (single-threaded ergonomics preserved); only bridge job/completion
types are `Send`. Interior mutability is confined to observably-pure memo
caches inside render helpers (§3 class 4).

**F2.6 — Elm-clone budget and layering.** Stated plainly: **no per-event
model cloning exists in this design.** Go-flavoured Elm copies models because
Go makes it cheap; here `update` mutates in place through `&mut`, so the
"Elm tax" is zero by construction — the F2.6 clone budget is "no clone",
not "bounded clone". The only sanctioned cloning is frame-buffer turnover,
bounded by screen area (cells), never by tree or state size. Reconciliation
vs. buffer-diff layering (§3.2 of feasibility): reconciliation (§2 + F2.3)
decides *which state survives and which subtree is dirty*; invalidation
(§5) decides *whether layout recomputes*; the buffer diff decides *which
cells cross the backend seam*. Neither layer pays twice when idle: `none`
skips `view`, clean subtrees skip layout, and the diff over an unchanged
buffer emits nothing — each layer is O(1)-ish on quiescent frames by a
different mechanism (skip flags, version counters, cell equality).

**Cell-model dependency (feasibility risk #1).** Component text APIs traffic
exclusively in `&str` / grapheme clusters with width-computed cursor offsets
(widths supplied by the width authority, never `char` counts); no component
contract names `char`-indexed cells. The grapheme-capable cell symbol type +
width/segmentation authorities are specified by the renderer RFC no later
than Phase 3 — recorded here as a dependency, not decided here, so no
`Cell::new('x')`-shaped API entrenches through component code first.

**Render pipeline end-state (Phase 1.6 resolution; normative).** The loop
runs six stages with explicit ownership at each step:

1. `describe` — components produce owned `View` descriptions from
   `(&Model, &Slot, &Context)`; no mutation, no I/O.
2. `reconcile` — framework matches descriptions against the generational
   slot table (paths + keys); mounts/drops settle here. A failed `describe`
   retains the previous `Desc` + key set: zero mounts, zero drops, frame
   aborted clean (joint rule with feasibility G1).
3. `layout` — framework borrows `&Desc` (+ viewport) into placements; user
   code performs no cell math.
4. `paint` — placements write into the single reused internal `&mut
   CellFrame`; this is the only mutation in the pipeline.
5. `diff` — frame buffer compared against the previous committed frame;
   only changed cells cross the backend seam.
6. `commit` — buffer published; `Frame` names this internal cell buffer
   exclusively and MUST NOT appear in the `Component` trait.

Change classes gate the pipeline: `none` skips describe/reconcile/layout/paint;
`repaint` reuses the previous layout (skips the layout stage only); `layout`
runs all stages. Per-frame description allocation is caller-owned and bounded
O(visible nodes); steady-state slot storage allocates nothing (allocation
baseline: 64 mounts → 64 slot boxes, 200 steady reconciles → +0 boxes,
0 drops — feasibility exp03).

## API sketches (all PROPOSED API — NOT YET IMPLEMENTED)

Preferred direction first; §"Alternatives" shows the rejected shape.
Brevity check (feasibility §4 bar: Ratatui hello-world ~10 lines, Cursive
~8, Bubble Tea counter ~60): the hello-world below is ~10 lines and the
counter ~15, with zero cell math and zero focus/identity boilerplate in app
code — meeting API_DESIGN principles 1 (intent over mechanics) and 3a
(build-and-run helpers) by construction, not aspiration.

```rust
// PROPOSED API — NOT YET IMPLEMENTED: hello world (preferred: owned-return view fn)
struct Hello;
struct App { name: String } // app model

impl Hello {
    // PROPOSED API — NOT YET IMPLEMENTED
    fn view(app: &App, ctx: &Context) -> Result<View, Error> {
        Ok(View::text(format!("hello, {}", app.name))) // semantic helper, not cells
    }
}
// run: App::new(App { name: "world".into() }).run()  // PROPOSED API — NOT YET IMPLEMENTED
```

```rust
// PROPOSED API — NOT YET IMPLEMENTED: counter (preferred: MVU triple)
struct Counter { count: i64 }
enum Msg { Incr, Decr }

fn update(model: &mut Counter, msg: Msg) -> Change {  // PROPOSED API — NOT YET IMPLEMENTED
    // returns BOTH Outcome + Change per update — carrier fixed by impl RFC (see §5 joint rule)
    match msg {
        Msg::Incr => model.count += 1,
        Msg::Decr => model.count -= 1,
    }
    Change::Repaint // RFC-1 Q13 name: state-only, no re-layout
}
fn view(model: &Counter, ctx: &Context) -> Result<View, Error> {  // PROPOSED API — NOT YET IMPLEMENTED
    Ok(View::text(format!("count: {}", model.count)))
}
```

```rust
// PROPOSED API — NOT YET IMPLEMENTED: stateful input (preferred: framework-owned slot)
struct Input { /* no user state */ }
struct InputState { draft: String, cursor: usize } // slot, keyed by identity

impl Component for Input {  // PROPOSED API — NOT YET IMPLEMENTED
    type State = InputState;
    type Message = InputMsg;
    fn init() -> Result<Self::State> { Ok(InputState { draft: String::new(), cursor: 0 }) } // fallible per §6 lifecycle
    fn update(slot: &mut Self::State, msg: Self::Message) -> Outcome { /* … */ }
    // returns BOTH Outcome + Change per update — carrier fixed by impl RFC (see §5 joint rule)
}
// parent reads committed value via emitted message, never via slot privates (question F)
```

```rust
// PROPOSED API — NOT YET IMPLEMENTED: dynamic list (preferred: explicit keys, owned nodes)
fn view(model: &TodoApp, ctx: &Context) -> Result<View, Error> {  // PROPOSED API — NOT YET IMPLEMENTED
    Ok(View::list(model.items.iter().map(|item| {
        View::row(&item.label).key(item.id) // state follows id across reorders
    }).collect()))
}
```

```rust
// PROPOSED API — NOT YET IMPLEMENTED: parent-child composition (preferred: owned nodes,
// heterogeneous sources erase into uniform View nodes at the return boundary)
fn view(model: &App, ctx: &Context) -> Result<View, Error> {  // PROPOSED API — NOT YET IMPLEMENTED
    // returns BOTH Outcome + Change per update — carrier fixed by impl RFC (see §5 joint rule)
    Ok(View::column(vec![
        Header::view(&model.title, ctx)?,
        TodoList::view(&model.todos, ctx)?, // child emits, parent decides (via messages)
        StatusBar::view(model.status(), ctx)?,
    ]))
    // heterogeneous children are held behind the erased render seam
    // (Box<dyn Render>-shaped element type, §8 F2.2) and resolve to owned
    // View nodes at return; the homogeneous View::column(vec![…]) spelling
    // above is the fast-path form, not the general mechanism.
}
```

> Shape note (feasibility P1/P2 synthesis): two mechanisms at distinct
> boundaries — erased render objects (`dyn`-compatible seam, §8 F2.2) at the
> child-holding boundary, homogeneous owned `View` nodes at the return
> boundary. The homogeneous fast-path enum avoids boxing for the common set;
> mixed lists fall back to boxing (mount-once rule, §8 F2.2). `column!`
> builder sugar, if added later, expands exactly to the above.

## Prior art

- **Ratatui (immediate-mode).** `Terminal::draw` closures recreate the UI each
  frame; `Widget`/`StatefulWidget` traits render into a `Buffer`, and a smart
  diff sends only changes. State lives entirely in user structs (`&mut State`
  passed per frame). Borrowed: pure view, buffer+diff renderer model. Rejected
  as the whole story: every app hand-rolls focus, identity ("same list,
  scrolled"?), and event bubbling — Ratatui maintainers and contributors
  explicitly note these retained-mode concepts are missing by design
  (ratatui.rs concepts: rendering, under-the-hood; discussion #552). Our
  identity layer + focus dispatch fill exactly that gap.
- **Bubble Tea / Elm (MVU).** `Model + Update(Msg) + View`, unidirectional
  data flow, nested-model composition by message routing (Charm docs; Parykin
  on nested models; leg100 on `Update`/`View` cost discipline). Borrowed: the
  loop shape, message taxonomy, "view is side-effect free" rule. Adapted: Go's
  value-receiver models become Rust `&mut` slots (no copying whole models per
  keystroke); `Cmd` becomes data-only `Effect` to keep core runtime-agnostic.
- **Cursive (retained, callback-driven).** A view tree with callbacks receiving
  the app handle (`cb_sink` for cross-thread updates). Borrowed: explicit focus
  order, retained identity intuition. Rejected as the whole story: callbacks
  with app-handle access break the unidirectional rule (sibling mutation,
  hidden call order), complicate headless testing, and trend toward "callback
  hell" (r/rust discussion). Our `Outcome`-returning handlers invert this:
  components describe, the loop performs.
- **Xilem / Masonry (reactive views over retained tree).** Lightweight,
  statically-typed view structs diffed to minimal retained-tree updates;
  reactive layer over a widget layer (Raph Levien 2022; xilem repo). Borrowed:
  the hybrid instinct itself — declarative views + retained updates — and the
  `Arc::make_mut`-style copy-on-write lesson for scoping mutation. Rejected at
  full fidelity: Xilem's granularity machinery (fine-grained dependency
  tracking, environment invalidation subtleties) is far more than a TUI core
  needs pre-`0.1.0`, and its async integration depth conflicts with our
  runtime-agnostic constraint.
- **Solid/Leptos-style signals (fine-grained reactivity).** Borrowed: the
  change-class intuition (only recompute what's dirty). Rejected as the core
  mechanism for Rust TUI: signal graphs need arena ownership + subscription
  bookkeeping that fights borrowck ergonomics, incentivise shared-ownership
  (`Rc`/`RefCell`) interiors, hide update order inside the graph (hurting
  determinism story C), and imply a global current-observer context — exactly
  the ambient state ARCHITECTURE.md §2.3 forbids.

## Decision matrix (reasoned, no bare scores)

Ratings are qualitative (strong / adequate / weak) with the reason inline.
Dimensions: API simplicity, ownership/borrowing, per-frame overhead, memory,
predictability, composition, events, debuggability, testability, focus,
errors, async compat, widget reuse, API stability, large/frequent-update UIs.

| Dimension | (1) Immediate-mode (Ratatui patterns) | (2) Retained tree (Cursive patterns) | (3) Elm/MVU (Bubble Tea concepts) | (4) Reactive signals | (5) Hybrid (PROPOSED) |
|---|---|---|---|---|---|
| API simplicity | Strong for static screens (draw closure, no framework types) but weak as apps grow: every app reinvents focus/identity; boilerplate scales with statefulness | Adequate: views feel concrete, but callback wiring + handle plumbing dominate real apps | Strong: three functions, one data flow; flattens learning curve for complete beginners | Weak in Rust: lifetimes across graph edges, memo boundaries, and effect discipline are expert terrain | Adequate-to-strong: MVU core keeps simple apps simple; keys/slots add concepts only when statefulness demands them |
| Ownership/borrowing | Strong: `&`/`&mut` per frame, no stored borrows | Weak: stored callbacks + shared handles push toward `Rc<RefCell>` | Strong in adaptation: `&mut` update + `&` view (avoids Go-style whole-model copies) | Weak: arenas + subscriptions + interior mutability fight the borrow checker | Strong: inherits MVU borrows; slots are plain owned values; frame descriptions own their data |
| Per-frame overhead | Adequate: full re-describe each frame, but diff bounds I/O; wasted layout work is the app's problem | Strong for idle (nothing recomputes uninvoked) but callback storms under load | Adequate: like immediate, bounded by view cost; update itself is cheap | Strong when quiet (surgical recompute), weak worst-case (graph bookkeeping, cascading memos) | Adequate-to-strong: MVU view cost + dirty-region culling + subscription-gated ticks |
| Memory | Strong: two buffers + user state, nothing else | Adequate: tree + handles retained; grows with UI size | Strong: model + previous frame | Weak: graph nodes, subscriber lists, memo caches retained per reactive site | Adequate: model + slots + previous frame; slots bounded by mounted set, dropped on unmount |
| Predictability | Weak: ordering/focus/state bugs are app-specific, untestable by the framework | Weak: callback order is emergent; reentrancy via handles | Strong: total event order, replayable scripts | Weak: glitch-avoidance and memo freshness rules are subtle | Strong: inherits MVU total order; batching cap makes pathological cases observable |
| Composition | Weak: nesting is manual function calls; no contract for children | Strong: tree nesting is natural | Adequate: nesting via message routing is explicit but verbose (the known nested-model tax) | Adequate: components compose, but implicit dependencies cross boundaries invisibly | Strong: tree nesting + typed child messages + keys; verbosity of MVU paid once per widget, reused freely |
| Events | Weak: decoding + routing are app code | Adequate: dispatch exists but ordering guarantees are thin | Strong: message taxonomy + single update entry | Adequate: events feed signals, but routing/focus need a second system anyway | Strong: §§3.4/4 give taxonomy + total dispatch + consumption rules |
| Debuggability | Adequate: state is yours, but "why this frame?" has no framework answer | Weak: callback chains resist tracing | Strong: event log fully explains state evolution | Weak: "which signal fired?" needs graph tooling | Strong: event log + change classes + damage-vs-diff debug assertions |
| Testability (E) | Adequate: view closures testable, but focus/routing untestable in-framework | Weak: handles need a live app instance | Strong: scripted messages → model asserts | Adequate: pure derivations testable; graph timing less so | Strong: pure view + scripted loop + golden frames, all headless |
| Focus | Weak: absent by design (the documented Ratatui gap) | Strong: first-class focus order | Adequate: app-managed, pattern-level | Weak: orthogonal; needs a parallel system | Strong: focus path + queryable ownership + declared moves |
| Errors | Adequate: user code, user errors | Adequate: callback errors lack uniform context | Strong: single update site ⇒ uniform taxonomy + path context | Weak: errors inside graph propagation lack a natural boundary | Strong: facade taxonomy + component-path context + teardown guarantee |
| Async compat (H) | Strong: nothing prescribed | Weak: `cb_sink` threads entangle runtimes | Adequate: `Cmd` pattern, but Go-isms don't transfer; Rust futures need a bridge | Weak: async signals/effects deepen executor coupling | Strong: data-only `Effect`, executor lives above core; sync loop is the baseline |
| Widget reuse (F) | Weak: "widgets" are render helpers needing app privates for state | Adequate: views reusable, but handle-coupled | Adequate: reusable via nested models, verbose | Adequate: reusable, but leak graph assumptions | Strong: slots give widgets private-by-construction state; parents interact via messages only |
| API stability (J) | Strong: tiny surface, nothing to break | Weak: handle/callback shapes ossify early | Strong: three-function shape is famously stable | Weak: signal/primitive choices are the hardest to change later (locks in granularity) | Adequate: trait has room (defaulted methods, `Outcome` extension) but identity/key semantics are load-bearing — flagged below |
| Large/frequent-update UIs | Weak: full re-describe + app-managed damage doesn't compose at scale | Adequate | Adequate: fast update, view cost dominates | Strong (its raison d'être) | Adequate-to-strong: damage tracking + tick subscriptions + paint/layout split; full signals-level granularity explicitly deferred |

Reading the matrix: options 1–4 each fail at least one load-bearing
requirement (1: identity/focus; 2: unidirectional flow + testability;
4: borrow ergonomics + no-globals + API stability). Option 3 alone satisfies
the flow/testability constraints but leaves identity, focus, and damage as
app-side patterns. The hybrid keeps 3's loop and adds exactly the retained
machinery needed for 1's gaps, without 4's graph. Its honest cost is
complexity: keys/slots/change-classes are more to learn than "just draw" —
mitigated by progressive disclosure (static screens never touch keys; paint is
the default change class).

## Alternatives considered (serious, rejected)

1. **Pure immediate-mode (Ratatui patterns end-to-end).** Rejected: pushes
   identity, focus, scroll-state, and bubbling into every app; framework cannot
   test what it does not own; ARCHITECTURE §8.8–§8.10 considerations would
   remain unaddressed by design. Sketch:

   ```rust
   // PROPOSED API — NOT YET IMPLEMENTED (REJECTED alternative — shown for contrast)
   loop {
       let ev = backend.read_event()?;
       app.handle(ev); // every app reinvents routing, focus, damage
       terminal.draw(|f| draw_all(&mut app, f))?;
   }
   ```

2. **Full signals reactivity.** Rejected: reasons in matrix (borrowing,
   hidden graph state, determinism, premature granularity lock-in). Revisit
   only via a new RFC with evidence that borrow ergonomics and the no-globals
   rule can both hold.

## Non-goals

- Exact trait method signatures and type names (implementation RFC within
  this contract).
- Renderer diff algorithm, layout solver, theme schema (sibling RFCs).
- Multi-threaded rendering, animation DSL, effect executor choice.
- Proc-macro syntax (requires its own RFC per §7).

## Open questions

Resolved in Round 2 (recorded, not re-litigated without new evidence):

- R1. Change classes: adopted RFC-1 Q13 (`none` / `repaint` / `layout`), no
  fourth class — former "Structure" reframed as identity output (§5).
- R2. Focus: single focus path at alpha; scopes postponed per RFC-1 (was OQ5).
- R3. Style/layout invalidation ownership: style declares the
  layout-affecting property list (empty in Phase 1, RFC-3 §6); the component
  loop treats theme/profile-version bumps as `repaint`-only (§5). No conflict.

Unresolved (genuine disagreements / implementer-detail, for Discussion):

1. Is `Outcome::Observed` (non-consuming observation) worth its complexity, or
   should consumption be strictly binary? (Carried; leaning drop unless a
   widget design evidences need.)
2. Should keys be enforced at compile time (e.g. via distinct builder types)
   for dynamic lists, or is runtime enforcement enough? Trade-off: safety vs.
   beginner friction. (Current position: required-by-contract at runtime with
   fallback flags (§2) — compile-time enforcement arguable in Discussion.)
3. `Send` bound on slot state: RESOLVED in Phase 1.6 — policy (b): no
   `Send`/`Sync` on models or slots; `Send` only on bridge job/completion
   types (evidence: E0277 pair in feasibility exp04; cross-reviewed).
   Single-threaded widgets keep `Rc`/`RefCell` ergonomics.
4. Effect cap value and overflow semantics (error vs. silent truncation with
   warning) — needs implementer measurement.
5. May `describe` and `layout` fuse for `repaint`-only subtrees
   (placement-`Vec` pooling)? Declared an implementation detail, not a
   contract change — implementer decides, contract text unchanged either
   way.
6. **Conflict flags:** none with RFC-1 (change classes adopted, focus
   resolved, J-list items addressed: identity §2, event extensibility §4,
   tick source §4, error taxonomy §6, remaining J items owned by sibling
   RFCs); none with RFC-3 (invalidation ownership split recorded as R3);
   none with ARCHITECTURE.md (as before). If the renderer RFC assigns
   cluster-splitting or cell-type duties to components, this RFC amends.

## Drawbacks

- More framework to learn than immediate-mode (keys, slots, outcomes, change
  classes) — mitigated by progressive disclosure and calm defaults.
- Trait-based components are more verbose than closures for trivial widgets —
  mitigated by closure shorthand for leaf handlers.
- Identity semantics (especially keyed/positional mixing fallback) are
  load-bearing API: getting them wrong is a breaking change later (question J)
  — mitigated by specifying misuse behaviour now and testing it headless.
- One-view-per-event bounds worst-case latency under message storms; the
  overflow cap converts hangs into errors but introduces a tuning parameter.

## Handoff (sprint deliverable, Round 2)

- **Preferred direction (unchanged):** Hybrid — MVU loop + retained identity
  (paths/keys) + pure view + data-only effects. Why: it is the only option
  satisfying all of unidirectional flow (§3.4 rule), headless testability (E),
  runtime agnosticism (H), widget reuse without privates (F), and deterministic
  replay (C), while directly answering considerations §8.8 (identity) and §8.9
  (invalidation) with testable contracts.
- **Rejected:** pure immediate-mode (no identity/focus/damage ownership);
  pure retained-callback (breaks unidirectional flow, weak testability);
  full signals (borrow ergonomics, hidden graph state, determinism cost,
  premature lock-in); Xilem-fidelity reactivity (overkill pre-`0.1.0`,
  async-depth conflict).
- **Scope/theming conflicts:** none outstanding. Adopted RFC-1's
  `none`/`repaint`/`layout` (R1), single-path focus (R2), and the J-list
  positions on event extensibility (`#[non_exhaustive]` + `Message(M)`
  wrapper) and tick source (injected value); accepted RFC-3's style-side
  invalidation ownership with component-side version-counter plumbing (R3).
- **Feasibility responses (F2.1–F2.6, all incorporated into §§7–8):**
  end-state render signatures (`&mut` update / `&`-only view, owned frame
  descriptions — no clone, no arena); `dyn`-compatible render seam with
  homogeneous fast path + mount-once boxing rule; hand-rolled generational
  slot table (zero-dep); borrow story with no `unsafe`; macro-free normative
  core with 4-method count justified; Elm-clone budget stated as "no clone"
  with reconciliation/invalidation/diff layering assigned per idle-cost
  mechanism. Cell-model work deferred to renderer RFC as a recorded
  dependency (§8).
- **Changes in this revision:** §5 rewritten to RFC-1 names (former
  "Structure" class removed); §4 gained extensibility mechanism + tick-source
  position + single-path confirmation; §7 gained macro-free commitment,
  method-count justification, seam order; new §8 feasibility end-state;
  sketches gained brevity check + `Repaint` naming; open questions split
  resolved/unresolved.
- **Contested items (for Discussion week):** (a) keys advisory vs. required
  — contract requires keys for dynamic lists at runtime (fallback + flags),
  but whether the *type system* should enforce it remains arguable and
  should be argued by a widget author; (b) `Send` bound — RESOLVED in
  Phase 1.6 to Send-at-boundary-only (evidence: E0277); reopen only with
  new implementer evidence; (c) `Outcome::Observed` survival — leaning drop.
- **Risks:** (a) identity/key misuse semantics are hard-to-change (J) — hold
  the Discussion week for widget-author review; (b) change-class precision
  depends on widget-author discipline — debug-build damage assertions are the
  backstop; (c) effect-bridge design could leak runtime types into core —
  enforce the §7 data-only rule in review of the lifecycle/effects RFC;
  (d) renderer RFC must deliver the cell-type replacement by Phase 3 or
  component text APIs face rework (§8 dependency).

## Amendment history

- 2026-10-16 (Round 1): Draft created (Phase 1.5 RFC sprint, Subagent 2).
- 2026-10-16 (Round 2): Cross-review revision — adopted RFC-1 change classes
  and focus scoping; recorded RFC-3 invalidation split; incorporated
  feasibility F2.1–F2.6 into §§7–8; open questions split into resolved (R1–R3)
  and unresolved (1–5). Status stays Draft.
- 2026-10-16 (Phase 1.6): Resolution amendments — owned-return pipeline made
  normative (§8.x rules 1–6; `&mut Frame` banned from the trait; sketches
  rewritten); keys required-by-contract at runtime (§2); Send-at-boundary-only
  policy adopted (OQ3 resolved); generational slot storage named (§8 F2.3);
  `Effect::Task` constrained to opaque routed data; allocation baseline
  recorded (64/64/+0); reconcile rule added (failed describe retains Desc).
  Evidence: `docs/research/RENDERING_MODEL_RESOLUTION.md`,
  `docs/research/COMPONENT_FEASIBILITY.md` (29 tests + quoted compiler
  proofs). Status stays Draft.
- 2026-10-16 (Phase 1.7): Review amendments — §1 loop text aligned
  (change-class names, Context inputs, no geometry in view); `Change`
  concept↔variant bridge + joint Outcome/Change carrier rule; sketches show
  both seam halves (erasure + owned nodes); init `-> Result<S, Error>`;
  `Effect::Task` Send-bound at bridge; OQ order/positions synced. Evidence:
  `docs/reviews/RFC2_FINAL_TECHNICAL_REVIEW.md`. Status stays Draft.
