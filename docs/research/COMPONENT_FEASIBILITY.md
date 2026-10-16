# Component Feasibility — Heterogeneous Children, Identity Storage, Send Policy

- Role: Subagent 2 — Component System & Rust Type-System Engineer, Jilva Phase 1.6.
- Repo: `D:\jilva\jilva-rs`, branch `phase1.6-architecture-resolution`. No commits.
- Read first: `docs/RFC/0002-component-model.md` (§2 identity, §4 events, §7
  ergonomics), `docs/ARCHITECTURE.md`, `docs/research/ARCHITECTURE_FEASIBILITY.md`
  (F2 findings, Round-2 reviews R2-C1–C12, X1–X6).
- Constraints honoured: MIT-only, edition 2024, stable 1.98.1, std-only
  prototypes (zero dependencies), no Tokio, no proc macros, no hidden globals,
  `forbid(unsafe_code)` in every experiment crate. Production crates untouched
  (`cargo build --workspace` still green).
- Outputs (exclusive): this doc + 4 experiment crates under
  `experiments/rfc-0002/heterogeneous-children/` (exp01–03) and
  `experiments/rfc-0002/threading/` (exp04). Each crate is detached (empty
  `[workspace]`), carries its own README (hypothesis / toolchain / commands /
  results / limits), and passes `cargo test` (29 tests total, exit 0).

> Claim discipline: every number below is either a hard test assert
> (fails the build if it rots) or is labelled REASONED. Compilation failures
> are evidence and quoted verbatim; the failing sources live in-repo but
> outside the build (`src/negative_*.rs`, no `mod` refers to them).

## 0. Prior-art continuity

This work starts where P3 (`jilva-proto-hetero`, ARCHITECTURE_FEASIBILITY §C
R2-C1) stopped: P3 proved `Box<dyn Component>` fails E0038 and a render-only
`dyn` seam compiles. The four experiments below extend that to the full
scenario matrix (Part A), the identity/storage contract (Part B), and the
`Send` policy (Part C) — the three BLOCKER-adjacent gaps R2-C1/C4/C5 named.

## Part A — Heterogeneous-children mechanisms

### A.1 Trait objects (`Box<dyn>`) — exp01 (8 tests green)

- The RFC-0002 sketch `column([Header, TodoList, StatusBar])` cannot compile
  as written. Two independent compiler proofs (stable 1.98.1):
  - array literal: `error[E0308]: mismatched types ... expected Header,
    found TodoList` (no common element type);
  - `fn erased(_: &dyn Component)` on the full trait: `error[E0191]: the value
    of the associated types State and Message must be specified`;
  - with assoc types bound:
    `fn erased(_: &dyn Component<State = String, Message = char>)` →
    `error[E0038]: the trait Component is not dyn compatible ...
    because associated function init has no self parameter`
    (same for `update`; rustc itself suggests the `where Self: Sized`
    quarantine).
- Positive: an object-safe render-only seam (`&self` methods, no generics, no
  assoc types, owned `String` return so no borrow escapes) holds heterogeneous,
  nested, conditional, dynamic, reordered, added, and removed children
  (tests 1–7). Incidental findings: `Box<dyn Render + Send>` needs `T: 'static`
  (E0310 otherwise); even a receiver-less `render(slot: &State)` trips E0038 —
  object safety demands a `&self` receiver, full stop.
- Positive (§A.3 boundary): `dyn SharedState<State = i64, Message = i64>`
  (receiver-less methods quarantined behind `where Self: Sized`) compiles and
  holds two DISTINCT widget types — proving `dyn` + assoc types is possible
  but forces ALL children to share one `State`/`Message` pair, i.e. it is
  homogeneous-only and unfit as the heterogeneity mechanism.
- Verdict: **trait objects are the correct render seam, but the trait must be
  split** — `dyn`-compatible render half (`&self`, object-safe) + separate
  state/update channel (exp03). A single `Component` trait cannot serve both.

### A.2 Enums — exp02 (tests 1–2, 5–6)

- `AnyWidget` enum dispatch renders all variants with ZERO heap allocation
  for composition. MEASURED (`size_of`, asserted): `AnyWidget` = 24B —
  niche-filled, no tag word (better than the reasoned 32B estimate);
  `Box<dyn Render>` = 16B stack + one heap allocation per child.
- Price, demonstrated: third-party `Fancy` composes via `dyn` freely but can
  NEVER inhabit `AnyWidget` without editing framework code (closed world).
- Verdict: **recommended as the homogeneous fast path** (framework-owned enum
  for the common set), never as the whole story — enum-only composition
  contradicts third-party widgets (RFC-0002 §7 seam order).

### A.3 Tuples — exp02 (test 3)

- `(A, B)` / `(A, B, C)` impls render heterogeneous fixed-arity rows with
  zero `Box`, static dispatch, stack values.
- Price: the arity wall (impl per arity; production needs N, movable with
  `macro_rules!` impl generation — allowed, declarative only).
- Verdict: **niche mechanism** for fixed small compositions (pairs, triples);
  not a general children story (no dynamic add/remove/reorder).

### A.4 Declarative macros — exp02 (test 4)

- `column![a, b, c]` expands to exactly `vec![Box::new(a) as Box<dyn Render>,
  …]`; macro output asserted byte-identical to the manual form.
- The macro-free expansion exists, compiles, and is tested — the RFC-0002 §7
  "macro-free normative core, macros strictly additive" commitment is
  demonstrated, not aspirational. No proc macros used or needed.
- Verdict: **approved as additive sugar**; the normative definition of every
  constructor MUST be its expansion (rustdoc renders the expansion, errors
  point at the expansion).

### A.5 Internal type erasure — exp03 (10 tests green)

- `Box<dyn ErasedSlot>` (`as_any`/`as_any_mut` + `type_tag`, blanket-implanted)
  behind a hand-rolled generational `SlotTable` (`Vec` entries + free list,
  `(parent, Key) → SlotId` map, positional `(parent, pos) → SlotId` map):
  all 10 scenarios pass — static, heterogeneous, nested (parent-scoped keys),
  conditional, dynamic add/remove with exact `(mounted, dropped)` counts,
  keyed reorder (state follows keys, zero remount), unkeyed reorder
  (state stays at position — the documented positional semantic),
  persistent local state (10 frames, `mounts == 1`), key collisions
  (first-wins + stateless ephemerals, factory cadence 1 mount + 1/frame),
  stale-handle rejection (generation bump, `get` → `None`), type replacement
  (`(1,1)` mount/drop, old id dead).
- `unsafe_code` is forbidden throughout: safe indexed access only — the R2-C5
  storage story needs no `unsafe`, no `SlotMap` dependency, no `Pin`.
- Verdict: **recommended as the state/update channel** complementing the
  A.1 render seam. Together they are the split-trait resolution of R2-C1:
  `dyn Render` for describing, erased slots for owning.

### A.6 Allocation map (measured vs reasoned — labelled)

HARD asserts (fail the build if behaviour changes), 64-child case, exp03 test 9:

```text
ALLOC-MAP: mounts=64 slot_boxes=64 steady_reconciles=200 added_boxes=0 capacities=(64,112,0) drops=0
```

| Phase | What allocates | Measured / reasoned |
|---|---|---|
| mount | one `Box<dyn ErasedSlot>` per heterogeneous child (exp01/03); enum/tuple children allocate nothing (exp02 `size_of` asserts) | MEASURED: 64 boxes for 64 children; backing capacities settle, then freeze |
| update (steady reconcile) | NOTHING framework-side: key/positional lookups only, no boxing, no map growth | MEASURED: +0 boxes, capacities `(64,112,0)` unchanged over 200 frames |
| render | borrows only (`&[Box<dyn Render>]`, `column` takes `&`); owned descriptions are caller-owned data | REASONED from signatures + test 7 (frames retained across renders); buffer-turnover cost is renderer-RFC territory |
| removal | deallocation only; the slot-drop point is where user `cleanup` runs | MEASURED: exact `dropped` counts (tests 3, 6) |
| collision extra | one ephemeral box PER FRAME, dropped end of frame — statelessness enforced by allocation cadence, not just convention | MEASURED: factory-call counting (test 10) |
| per-frame descriptions | `Vec<ChildDesc>` rebuilt per frame, O(children) — caller-side owned data, bounded, NOT eliminated and NOT slot storage | REASONED (by construction); explicitly distinct from the zero-steady-state slot claim |

Net: the RFC-0002 §8 "at most one Box per heterogeneous child, allocated once
at mount and reused" rule is implemented and measured. The homogeneous fast
path (enum/tuple) allocates nothing at all.

## Part B — Identity contract: keys RECOMMENDED (not optional, not required)

Recommendation: **keys are REQUIRED by contract for dynamic collections,
advisory for static structure — with the requirement enforced observably, not
by the type system.** Concretely:

1. Static structure, no keys → positional identity (exp03 test 5 semantics).
2. Dynamic collections (add/remove/reorder/length-change) → keys REQUIRED:
   an unkeyed dynamic list keeps compiling but its state follows POSITIONS,
   which after a reorder is observably wrong state placement (demonstrated:
   swap test keeps `n:1` at position 0). The framework MUST surface this:
   length-change or reorder on an unkeyed list sets a `positional_dynamic`
   outcome flag (same flag family as exp03's `mixed_keys_fallback`).
3. Keyed ↔ unkeyed mixing within one sibling list → forbidden: fall back to
   positional for the whole list + set `mixed_keys_fallback` + count it
   (exp03 test 8). Debug builds `eprintln!`; release never panics.
4. Key collision → first wins; extras get fresh stateless ephemerals each
   frame + per-frame `collisions` count + cumulative counter (exp03 tests 4,
   10). Never panic in release; `debug_assert` the uniqueness invariant only
   where the framework (not user data) is at fault.
5. Type change at a live identity → old slot dropped with cleanup, new slot
   mounted; stale handles return `None` (exp03 tests 6–7). Type change ⇒ new
   component, even if key/position match (RFC-0002 §2 table, unchanged).
6. Conditional branches → positional identity; branch-independent state needs
   keys or lifted model state (RFC-0002 §2, unchanged).

Why "required-by-contract, advisory-by-compiler": compile-time required keys
(distinct builder types) were prototyped mentally against the evidence and
rejected — they punish static screens (progressive-disclosure cost in the
decision matrix), complicate the `column!` sugar signature, and cannot cover
ordering mistakes anyway (keys present but unstable across frames is the
real-world corruption source, and no type system catches it). Runtime
observable behaviour catches ALL FOUR misuse classes uniformly; the type
system catches none of them completely. The contract table above is the
enforcement.

## Part C — Send policy: `Send` at the async boundary only

Recommendation: **(b) `Send` at the async boundary only** — core loop, models,
and slots carry NO `Send`/`Sync` bounds; the effect bridge requires `Send`
solely on job closures and completion values that actually cross threads.
(Evidence: exp04, 5 tests green + 2 E0277 negative proofs.)

- Universal-`Send` (policies c/d) rejects `Rc<RefCell<Vec<String>>>` models
  and `Rc` slot memos with `error[E0277] ... cannot be sent between threads
  safely` — quoted verbatim for both. That error is what every
  single-threaded widget author (input drafts, memoised windows, shared
  caches — the RFC-0002 §3 "observably pure memo" shape) would hit, while the
  single-threaded loop gains NOTHING from the bound (it never moves state
  across threads; it has no executor by ARCHITECTURE §2.4).
- The bridge shape is proven sufficient: `Bridge::spawn_job<F, Out>`
  (`F: Send`, `Out: Send`) + join + apply-on-loop-thread works end to end
  with `!Send` state resting on the loop thread throughout (test 4).
- `Sync` is required NOWHERE: the loop holds one `&mut` on one thread;
  shared borrows never cross threads. (`Cell<usize>` is `Send` but `!Sync` —
  positively asserted — and needs no policy either way under (b).)
- R2-C9 executor-by-stealth is closed by construction: `Effect::Task` is an
  opaque `Box<dyn FnOnce() -> M + Send>` that core routes WITHOUT executing,
  spawning, or naming threads (test 5: `route_without_executing` is an
  identity move). Execution policy lives above core with the bridge; alpha
  can further restrict effects to `After`/`None` data without touching the
  seam.
- No Tokio, no runtime types in any signature; `std::thread` appears only as
  the bridge stand-in.

## Handoff

### Recommended mechanisms

1. Heterogeneous children: **split seam** — object-safe `Render`
   (`&self`, owned returns) behind `Vec<Box<dyn Render>>` for the general
   case (exp01) + framework-owned enum fast path with `Boxed` fallback for
   the common set and third-party escape hatch (exp02 `Child`) + tuple impls
   for fixed small arities + `macro_rules!` sugar over a tested macro-free
   core. All five evaluated; each has a named place, none pretends to be
   the whole story.
2. State/identity: **framework-owned generational slot table + erased slots**
   (exp03), zero-dep, no `unsafe`, mount-once boxing measured, misuse
   observable via outcome flags.
3. Keys: **required-by-contract for dynamic collections, enforced at runtime
   via flags** (Part B table); static structure stays positional and key-free.
4. Threading: **no universal `Send`; `Send` at the async-bridge boundary
   only** (Part C table, policy (b)).

### Allocation map (one line per phase)

Mount: 1 box/heterogeneous child (+ table backing, settles then freezes) |
steady update: 0 framework allocations (measured over 200 frames) |
render: borrows only | removal: drops + cleanup point |
collision extra: 1 ephemeral box/frame (statelessness by construction) |
per-frame descriptions: caller-owned O(children), bounded, distinct from slots.

### RFC-2 patch list (for the RFC-2 author flow via orchestrator — NOT applied here)

1. P1 (R2-C1): replace the `column([Header, TodoList, StatusBar])` sketch with
   the explicit-erasure form (`Vec<Box<dyn Render>>` element type named) and
   name the split seam (§8 F2.2): object-safe render half + erased slot
   channel. Cite exp01 E0308/E0191/E0038 chain + exp02 hybrid.
2. P1 (R2-C1/C2-adjacent): state whether `view` returns owned descriptions or
   writes into `&mut Frame` NORMATIVELY and rewrite all sketches to match —
   flagged by feasibility, mechanism evidence for the owned form in exp01
   test 7 / exp03 desc ownership; final call stays with the RFC-2 author
   (rendering-model sibling may own the `Frame` side — coordinate, don't duplicate).
3. P1 (Part B): rewrite §2's "SHOULD carry a Key" as the required-by-contract
   table (§B.1–6 above): unkeyed-dynamic flag, mixing fallback + flag,
   collision first-wins + stateless + count, type-change replacement, stale
   handle `None`. Cite exp03 tests 2/4/5/7/8.
4. P1 (R2-C4 / open Q3): change §7 "`Model: Send` is required" to policy (b):
   no `Send`/`Sync` on models/slots; `Send` only on bridge job/completion
   types. Cite exp04 E0277 pair + bridge test.
5. P2 (R2-C5): name the storage family in §8 F2.3 — hand-rolled generational
   `Vec` table, `(parent, Key)` map, `Box<dyn ErasedSlot>` states, `Any`
   downcast recovery, per-sweep cleanup point. Cite exp03 (10 tests, no
   `unsafe`, no new dep).
6. P2 (R2-C9): constrain `Effect::Task` to opaque routed data executed above
   core (or restrict alpha effects to timer data); core names no thread,
   executor, or spawn point. Cite exp04 test 5.
7. P3: record the allocation rule with the measured numbers
   (64/64/+0-over-200-frames) as the regression baseline the implementation
   RFC must not regress.
8. Cross-check with P2's author (rendering-model): ownership of per-frame
   description allocation + `Frame` shape, so §8's "never per-frame boxing in
   steady state" and the renderer diff story share one vocabulary.

### Recall for cross-review

- Cross-review pointers: R2-C1 (heterogeneity) → exp01/02 READMEs + §A.1–A.5;
  R2-C4 (Send) → exp04 README + §C; R2-C5 (storage) → exp03 README + §A.5;
  open Q2 (keys) → §B; open Q3 (Send) → §C; F2.2 → §A.6 map; F2.3 → §A.5.
- Reproduction: `cargo test` in each of the four crates (29 tests, exit 0);
  negative proofs: the two `rustc --edition 2024 --crate-type=lib
  src/negative_*.rs` commands (expected failures E0308/E0191/E0038, E0277×2).
- Scratch check file (outside repo, Temp only):
  `C:\Users\hamro\AppData\Local\Temp\opencode\exp01-bound-dyn-check.rs`
  (bound-assoc-types → E0038 step).
- Limits carried forward: no deep-tree stress test; no dispatch benchmarks;
  no Tokio-specific evaluation (out of scope by ARCHITECTURE §2.4); `Frame`
  shape left to the rendering-model track (P2 above).

## Amendment history

- 2026-10-16: created (Phase 1.6, Subagent 2). 4 experiment crates, 29 tests
  green, 5 quoted compiler proofs (E0308, E0191, E0038 ×2 chains, E0277 ×2),
  1 measured allocation line, Parts A–C + handoff. Status: final for this
  workstream; RFC-2 edits belong to the author flow.

---

# Cross-review — Agent 1 rendering resolution vs. this component feasibility (2026-10-16)

- Reviewer: Subagent 2 (component track). Reviewed: `docs/research/
  RENDERING_MODEL_RESOLUTION.md` (Agent 1) + skimmed
  `experiments/rfc-0002/rendering-model/src/main.rs` (19 checks, std-only).
- Question asked (§6 tasking): do Agent 1's ownership/lifetime assumptions
  support dynamic composition (heterogeneous children, mid-pipeline
  mount/unmount, generational slots, keyed reconciliation)? Does
  owned-View-per-node survive §A.6's allocation map? Does the internal
  mutable buffer + fail-clean rule conflict with drops accounting?
- Verdict: **COMPATIBLE with two gaps and one self-correction.** No
  load-bearing contradiction. Agent 1's pipeline (C: owned `describe` →
  borrowing `layout` → internal reused `&mut CellFrame`, `Frame` banned from
  the `Component` trait) is the render-side complement of this doc's split
  seam (`dyn Render` + erased slots). The gaps are both ordering problems at
  the seam between the two tracks, fixable with one pipeline rule.

## Agreements (with pointers)

1. **Owned seam, same shape.** Agent 1 §5.1 (`view` takes `&` only, returns
   `Result<Description, Error>`) is exactly the object-safe `&self`-only
   remainder this doc's exp01 test 8 isolates as the dyn-compatible half.
   Their `Node`/`Desc` tree with `key: Some(id)` as data (checks #6–#7) is
   the caller-owned description side my exp03 `ChildDesc` consumes. The seam
   between tracks: heterogeneous widgets resolve to homogeneous `Desc` nodes
   at the return boundary (`parent` embeds child's returned `Node` — their
   §3 table). No overlap, no conflict.
2. **Two different rejections of `&mut Frame`, same outcome.** Their B1/B2
   rejection rests on measured error semantics (orphan nodes check #11,
   half-painted buffers check #13) and purity (API_DESIGN §1); mine rests on
   E0038 dyn-incompatibility of receiver-less methods. Note for precision:
   a `&mut Frame` *parameter* is itself object-safe — the ban stands on
   their grounds, not mine. Complementary, not competing.
3. **Borrows are pipeline-scoped on both sides.** Their `Placed<'d>`
   borrows `Desc` text inside one `render()` call (`main.rs:328`); my slots
   own erased boxes and store no borrows. Neither borrow crosses the
   event→update→render transaction; neither touches slot storage. The
   "no lifetimes escape" contract holds across both tracks.
4. **Allocation stories agree once stated precisely.** Their release-profile
   bytes (A: 3,809,000 B/1000 renders × 50 rows; C describe+layout 6,445,000;
   paint ≈20 B/render into the reused buffer) measure the DESCRIPTION layer
   (owned strings per node per frame, ~76 B/row). My debug-profile structural
   counts measure the SLOT layer (64 boxes at mount, +0 over 200 frames).
   Different layers, different profilers — both say the same structural
   thing: per-frame cost is O(visible children/rows), never tree- or
   state-sized. Paint-at-20B supports the F2.6 "buffer turnover bounded by
   screen area" line both docs cite.
5. **Methodology agreement on instrumentation.** Agent 1's counting allocator
   is explicitly harness-only (`main.rs:16-40`); I removed my own counting
   allocator for the same `forbid(unsafe_code)` reason and used ledger +
   capacities. Both docs warn their numbers order candidates but are not
   publishable framework claims (their §8.5; my "claim discipline" header).
6. **Send policy untouched = compatible by silence.** Agent 1 proposes no
   `Send`/`Sync` bounds anywhere; nothing in the C pipeline requires them
   (`describe`/`layout` take `&`, `paint` takes `&mut` on one thread).
   Policy (b) stands unopposed.
7. **Their keyed-reorder check is subsumed, not contradicted.** Their
   `reconcile` (`main.rs:156-163`) is a `HashMap<u64, usize>` sighting
   counter: it proves a key survives a swap (check #7, count reaches 2) but
   models no typed state, no sweep/unmount, no generations, no collisions —
   and it never removes keys, so it cannot validate drop accounting. Exp03
   tests 2/3/7 strictly subsume it. Identity evidence lives here; theirs is
   a harness simplification, correctly scoped.

## Gaps and contradictions

- **G1 (GAP, joint patch required): reconcile-vs-pipeline ordering is
  unspecified, and failed-describe slot behaviour is undefined.** Agent 1 §5
  orders describe → layout → paint with Err aborting before paint — but says
  nothing about the reconciliation sweep (mount/drop + cleanup). My drops=0
  steady-state accounting assumes reconcile runs against a successfully
  produced description. If an implementation sweeps against a missing/partial
  `Desc` after a failed describe, slots unmount spuriously, `cleanup` runs
  spuriously, and BOTH the fail-clean rule and the drops ledger break.
  Required rule (proposed jointly): pipeline order is
  `describe → reconcile (mount/drop+cleanup) → layout → paint`; a failed
  describe retains the previous `Desc` AND the previous key set, so
  reconciliation is a no-op (zero mounts/drops) — fail-clean extended from
  buffers to slots. This also fixes unmount/paint interleaving: cleanup
  side effects can never touch an in-flight paint because paint only ever
  sees the post-reconcile `Desc`.
- **G2 (SELF-CORRECTION to this doc, §A.6): the "render: borrows only" row
  under-states user-side allocation.** Exp01 test 7 proves no borrow
  *escapes*; it does not prove describe allocates nothing — Agent 1 measures
  the owned-`String`-per-node copy-out (§3 table, check #4) at ~76 B/row.
  Corrected reading, adopted as patch delta P7 below: per-frame `Desc`
  allocation O(visible children) is EXPECTED and bounded (their number is
  the baseline); the zero-steady-state claim covers FRAMEWORK slot storage
  only (my ledger). The two numbers compose; neither contradicts the other.
- **G3 (minor, theirs): §8 open Q3** (whether `View` piggybacks change hints
  vs update-only returns) interacts with Part B outcome flags: if `describe`
  returns per-subtree hints, mixing/collision flags could ride the same
  channel. Coordination item for the implementation RFC, not a decision.
- **No other contradictions found.** Buffer ownership (§3.7) vs slot
  ownership (§3.3) are different owners at different layers (F2.6 layering,
  cited by both); internal `&mut CellFrame` never meets `&mut Slot` (one
  `&mut` per layer per frame — the borrow story holds).

## Patch-list deltas (changes to this doc's Handoff patch list)

- **P1 (mine) + their §6.3 SYNTHESISED (not competing):** the rewritten
  sketch is `fn view(&self, ctx: &Context) -> Result<View, Error>` where
  heterogeneous children are `Vec<Box<dyn Render>>`-erased widgets each
  returning owned nodes — my explicit-erasure form inside their owned-return
  form. Either patch list alone under-specifies the sketch; together they
  pin it. Request the RFC-2 author apply both as one edit.
- **P2 AMENDED:** add the G1 pipeline-ordering rule (describe → reconcile →
  layout → paint; failed describe ⇒ retain previous `Desc` + key set ⇒ zero
  mounts/drops). Jointly owned with Agent 1's §6.2; neither track can land
  it alone.
- **P7 AMENDED:** allocation baseline quotes BOTH figures with profiles
  labelled (their release bytes for descriptions + paint; my debug
  structural counts for slots) and states per-frame `Desc` O(visible rows)
  as expected (G2 correction). The implementation RFC regresses neither.
- **P8 AMENDED:** add G3 coordination (misuse flags vs View-returned change
  hints) for the implementation RFC.
- **P4 UNCHANGED, with a note to Agent 1:** recommend their
  implementation-RFC handoff adopt Send-policy (b) explicitly, since their
  pipeline is already compatible with it by construction.
- **Their §6.1/§6.4–§6.6/§7/§8:** endorsed as written; no component-track
  objection. Their §8.1 (exact `Description` encoding deferred to renderer
  RFC) is the right call and also bounds my P5: slot storage is encoding-
  agnostic (keys + `type_tag` + erased boxes work against any node shape).

## Handoff (cross-review)

- Compatible: owned user seam, `Frame`-ban (different grounds, same ban),
  pipeline-scoped borrows, layered allocation stories, Send-silence,
  harness-only instrumentation, subsumed slot-survival check (§Agr.1–7).
- Incompatible: nothing load-bearing. Two gaps (G1 ordering/fail-clean
  slots — joint patch; G3 hint-channel coordination) and one
  self-correction (G2 render-allocation row).
- Patch deltas: P1 synthesised with their §6.3; P2 + P7 amended; P8
  extended; P4 carried with an adoption note to Agent 1.
- Reproduction unchanged: 29 component tests + 19 rendering checks; negative
  proofs as listed in each README. No files outside
  `docs/research/COMPONENT_FEASIBILITY.md` touched by this review.
