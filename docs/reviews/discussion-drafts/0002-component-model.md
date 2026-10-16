# [RFC-0002 Discussion] Hybrid component model (MVU + retained identity)

> Status of the proposal under discussion: **Draft**
> (`docs/RFC/0002-component-model.md`, 2026-10-16, Round-2 revision).
> Per `docs/RFC/README.md`, no decision before 7 full days from this thread's
> posting timestamp. Nothing here is accepted.

## Problem

ARCHITECTURE.md §3.3 leaves the component flavour open but fixes what must be
specified: identity, state ownership, event handling, composition (plus
API_DESIGN.md §4: misuse behaviour, state rule, bubbling/focus order, pure
render). Without this RFC, implementers improvise each and users unlearn them
later. Widget authors additionally need reusable stateful widgets without
access to app privates, all headless-testable.

## Options considered

Full decision matrix in the RFC; summary: (1) pure immediate-mode — no
identity/focus/damage ownership, rejected; (2) retained-callback tree — breaks
unidirectional flow, weak testability, rejected; (3) Elm/MVU alone — satisfies
flow/testability but leaves identity, focus, damage as app patterns;
(4) fine-grained signals — borrow-ergonomics, hidden graph state, determinism
cost, premature lock-in, rejected; (5) **proposed hybrid** — MVU loop
(events → update → view, one event → one transactional update → ≤1 frame) over
app-owned state, plus retained identity (positional paths + explicit keys),
framework-owned ephemeral slots, pure view returning owned frame descriptions,
`#[non_exhaustive]` event taxonomy + `Message(M)` wrapper, data-only `Effect`,
change classes none/repaint/layout adopted from RFC-1.

## Consequences

More to learn than "just draw" (keys/slots/outcomes/change-classes — mitigated
by progressive disclosure); trait verbosity for trivial widgets (closure
shorthand mitigates); identity semantics are load-bearing API (misuse behaviour
specified + headless-tested now); one-view-per-event bounds storms via a
tuning-parameter overflow cap.

## Recommendation going in (provisional — maintainer decides after Discussion)

Accept the hybrid: §§1–8 contracts as the component bounding box, with the three
contested items below settled by this thread's evidence.

## Evidence

- Feasibility F2.1–F2.6 incorporated as normative §8: `&mut` update / `&`-only
  view, owned frame descriptions ("no clone" Elm budget); `dyn`-compatible
  render seam with homogeneous fast path + mount-once boxing; hand-rolled
  generational slot table (zero-dep); no-`unsafe` borrow story; macro-free
  normative core, 4 required methods justified vs Cursive's `View`.
- Brevity check: hello-world ~10 lines, counter ~15, zero cell math.
- Resolved R1–R3: RFC-1 change classes adopted verbatim; single focus path at
  alpha; style/layout invalidation authority split with RFC-3.

## Specific questions for reviewers (the contested items — please argue these)

1. **Keys: advisory-with-runtime-fallback (current) vs required
   (compile-time)?** Safety (J-list: identity is expensive to change) vs
   beginner friction. New evidence refines the positions: Phase-1.6 recommends
   required-BY-CONTRACT for dynamic collections (unkeyed dynamic lists keep
   compiling but state follows positions — observably wrong after reorder —
   so the framework MUST surface a `positional_dynamic` flag), advisory for
   static structure; compile-time-required keys rejected (punish static
   screens, can't catch unstable-across-frames keys — the real corruption
   source). Widget authors especially wanted: is runtime-flag enforcement
   sufficient, or do you hold the compile-time position?
2. **`Send` bound on slot state (current: required) vs `!Send`-tolerant slots
   with `Send`-only opt-in for bridged effects?** New evidence recommends
   policy (b): NO `Send`/`Sync` on models/slots, `Send` only on bridge
   job/completion types that cross threads — universal-`Send` rejects
   `Rc<RefCell<…>>` models and `Rc` slot memos with E0277 (quoted verbatim ×2)
   while the single-threaded loop gains nothing (never moves state across
   threads, no executor per ARCHITECTURE §2.4); bridge shape proven with
   `!Send` state resting on the loop thread. Confirm policy (b)?
3. **`Outcome::Observed`** (non-consuming observation): worth the complexity or
   strictly binary consumption? Author leans drop unless a widget design
   evidences need — bring the design or second the drop.
4. Effect-cap value and overflow semantics (error vs silent truncation with
   warning) — needs implementer measurement; propose numbers.
   Constraint (new evidence): `Effect::Task` must stay opaque routed data
   executed above core — core names no thread, executor, or spawn point
   (executor-by-stealth closed by construction). Alpha may further restrict
   effects to `After`/`None` data without touching the seam.
5. `Tick` as injected plain-data timestamp (components clock-agnostic): does the
   pending lifecycle RFC need anything more from this contract?
6. **Render seam (new Phase-1.6 evidence — `experiments/rfc-0002/rendering-model/`,
   19/19 checks green): owned `describe` + framework pipeline (recommended C)
   vs `&mut Frame` in the trait?** Problem: RFC-0002's normative text says
   owned descriptions (`view` takes `&` only, F2.1) but all five sketches show
   `fn view(model, frame: &mut Frame)` — implementers split two incompatible
   ways, and `Frame` is never defined as builder vs cell buffer. Options: (A)
   owned return `view(&self, ctx) -> Result<View, Error>`; (B1) `&mut` builder —
   measured orphan nodes on `Err` (discard-on-`Err` burden on every widget);
   (B2) `&mut` cell buffer — user code does geometry (API_DESIGN §1 violation),
   half-painted buffers on `Err`; (C, recommended) A's seam + framework-owned
   `layout` (borrowing, zero string copies) + internal reused `&mut` cell
   buffer, `Frame` banned from the `Component` trait, no-partial-output rule
   (`Err` before paint → previous buffer retained, diff emits nothing), change
   classes gate stages (`none` skips all, `repaint` skips layout).
   Consequences: sketch rewrite + one new subsection, no §§1–7 semantic change;
   unblocks renderer + implementation RFCs. Evidence: §3 table in
   `RENDERING_MODEL_RESOLUTION.md` (composition, error, determinism, alloc:
   paint ≈ 20 B/render proving buffer reuse). Recommendation: adopt C.
   Questions: `View` vs `Description` name (bikeshed in implementation RFC —
   confirm)? Should `view` also return `Change` alongside `View`, or does
   `update`'s return stay the sole invalidation source? Describe/layout fusion
   for repaint-only subtrees as pure implementation detail — confirm?
7. **Heterogeneity mechanism (new evidence — 4 crates, 29 tests, quoted
   compiler proofs): split seam vs single-trait vs enum-only?** Problem:
   `column([Header, TodoList, StatusBar])` cannot compile as sketched (E0308
   array mismatch, E0191 assoc-type binding, E0038 non-dyn-compatible `init`/
   `update`). Options: single `Component` trait as `dyn` (proven impossible);
   enum-only dispatch (zero alloc — measured 24B niche-filled — but closed
   world, third-party widgets locked out); recommended split seam: object-safe
   `Render` half (`&self`, owned returns) behind `Vec<Box<dyn Render>>` +
   erased-slot state channel (hand-rolled generational table, zero-dep, no
   `unsafe`) + framework enum fast path with `Boxed` fallback + tuple impls
   for fixed arities + `macro_rules!` sugar over a tested macro-free core.
   Consequences: trait must be split (render half vs state/update channel);
   allocation rule "≤1 Box per heterogeneous child, mount-once" measured
   (64 boxes/64 children, +0 over 200 steady frames) becomes the regression
   baseline. Evidence: `COMPONENT_FEASIBILITY.md` Parts A–B. Recommendation:
   adopt the split seam. Question: does the `dyn Render` + erased-slot split
   satisfy third-party-widget authors, or is the enum fast-path boundary wrong?
