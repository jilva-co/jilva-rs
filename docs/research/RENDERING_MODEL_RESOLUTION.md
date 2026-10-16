# Rendering Model Resolution (RFC-0002 Phase 1.6)

- Status: Resolution proposal (Draft — for cross-review; does NOT amend RFC-0002 by itself)
- Author: Subagent 1 (Rendering Contract & Ownership Architect, Phase 1.6)
- Date: 2026-10-16
- Scope: `docs/RFC/0002-component-model.md`, `docs/ARCHITECTURE.md` §3, `docs/API_DESIGN.md`
- Evidence: `experiments/rfc-0002/rendering-model/` (compile-tested, std-only, stable 1.98.1)

## 1. The contradiction (as found)

RFC-0002 contains two incompatible render signatures:

| Location | Signature | Meaning |
|---|---|---|
| Normative text: §3 ("No lifetimes escape: frame descriptions are **owned values**"), §8 F2.1 ("`view` takes `&` only. There is deliberately **no `&mut self` render** … `view` returns nothing by reference") | Owned `fn view(&self, ctx: &Context) -> View` (i.e. **Approach A**) | User code returns an owned description tree; no `&mut` anywhere on the view path |
| All five API sketches (hello-world, counter, input, dynamic list, composition) | `fn view(model: &App, frame: &mut Frame)` (i.e. **Approach B**) | User code pushes into a caller-owned mutable `Frame` |

The sketches are labelled "PROPOSED API — NOT YET IMPLEMENTED", but so is the
normative prose's claim; a reader implementing from the sketches builds B while
a reader implementing from §§3/8 builds A, and the two do not interoperate
(ownership contracts, error semantics, and test doubles all differ). This
document resolves the contradiction with tested evidence.

**Terminology fix applied throughout:** the word `Frame` is used in the RFC
sketches without ever saying whether it is a *description builder* or a *cell
buffer*. Those are different types with different owners (see §4). The
resolution reserves each meaning explicitly.

## 2. Candidates evaluated

- **A — owned description:** `fn view(&self, ctx: &Context) -> Result<View, Error>`
  where `View` is an owned tree (regions + styled spans as data, no borrowed
  lifetimes). Matches RFC-0002 §§3, 8/F2.1 and API_DESIGN.md §4 ("Purity of
  render").
- **B1 — `&mut Frame` as description builder:** `fn view(&self, ctx, frame: &mut DescFrame)`.
  Same information as A, pushed incrementally instead of returned.
- **B2 — `&mut Frame` as cell buffer:** `fn view(&self, ctx, frame: &mut CellFrame)`
  with `set_cell`/`text`-at-coordinates. User code performs geometry.
- **C — hybrid (recommended):** user seam is exactly A (`describe` returns owned
  `Desc`); the framework then runs `layout(&Desc) -> Vec<Placed<'_>>`
  (borrowing `Desc` text, zero string copies) and `paint(&[Placed], &mut CellFrame)`
  into a **framework-owned, reused** cell buffer. The only `&mut Frame` in the
  system is internal to the renderer; it never appears in the `Component` trait.

## 3. Evidence (all compile-tested, exit 0)

Harness: `experiments/rfc-0002/rendering-model/` — single std-only crate,
detached via empty `[workspace]`, `cargo run --release`, 19/19 checks pass on
stable 1.98.1. Each approach renders the same fixture (title + N keyed rows)
through: composition/nesting, borrowed model data, dynamic children, keyed
reorder with slot survival, fallible render, and repeated-render determinism.
Allocation probe is a counting `#[global_allocator]` (harness-only, not proposed
design) measuring bytes allocated over 1000 renders × 50 rows of equivalent work.

| Property | A (owned return) | B1 (`&mut` builder) | B2 (`&mut` cells) | C (hybrid) |
|---|---|---|---|---|
| Composition/nesting | `parent` embeds child's returned `Node`; `?` composes (checks #1–#3) | Sequential pushes; nesting is manual call order | Same as B1, plus manual y-advance | Same as A at user seam (#14) |
| Borrowed model data | `&Item` → owned `String` at seam; description outlives borrow (#4) | Same copy-out | Same copy-out | Same copy-out |
| Dynamic keyed children | `map` + `collect` into owned `Vec`; keys are data (#6–#7, slot survives swap) | Push loop; keys are data | Push loop; keys bypassed (position = paint order) | Same as A |
| Error propagation | `Err` returns **nothing** — no partial output exists (#8–#9) | `Err` leaves **orphan nodes** in caller frame; measured: 1 orphan on failed render (#10–#11) → contract must mandate discard-on-`Err` | `Err` leaves **half-painted cells** as the only output; nothing to diff (#13) | `Err` in describe/layout skips paint; buffer untouched, previous frame intact, diff emits nothing (#16) |
| Repeated renders | Bit-identical owned values, trivially assertable (#5) | Assertable only after successful build | Requires cell-by-cell comparison; geometry baked in | Bit-identical cell buffers (#17–#18) |
| Measured alloc (B/1000 renders) | 3,809,000 | — (same shape as A + discard burden; not separately benchmarked) | — (buffer sized by viewport, but geometry in user code) | describe+layout 6,445,000; full render w/ reused buffer 6,465,480 (paint adds ≈20 B/render) |

Reading the numbers honestly: C's describe+layout costs ~1.7× A here because the
prototype keeps `Desc` (owned strings) **plus** a `Vec<Placed>` of borrowed
placements — one extra small `Vec` per render, bounded by *visible rows*, never
by tree or state size. `layout` performs zero string copies (it borrows `Desc`
text; verified by the lifetime structure, not just the counter). Paint into the
reused buffer is ~20 B/render — buffer reuse works. An implementation may
additionally pool the placement `Vec` or fuse describe+layout; both are
framework-internal optimisations invisible to users, which is exactly the point:
under C, allocation strategy is the framework's business, not the user's
signature.

## 4. Why B is rejected (both senses of `Frame`)

**B1 (builder) is A with worse error semantics.** It carries identical
information to A but converts total error propagation (`Result<View>`) into a
partial-mutation protocol: every `view` must document what a failed call left
behind, and every caller must discard-on-`Err`. The prototype *measures* the
orphan (check #11). This is pure contract surface with zero compensating
benefit at TUI scale — the "avoid one intermediate tree" argument saves one
bounded-by-viewport allocation while adding a discard rule to every widget
ever written. It also breaks the RFC's own F2.1 ("no `&mut` render") and §3
("no lifetimes escape" becomes "no garbage escapes", an unenforceable rule).

**B2 (cell buffer) violates API_DESIGN.md §1 in user code.** "Anything requiring
manual cell math from app code is an API bug" — yet B2's `text()` performs
y-advance, truncation, and wrap-or-clip decisions inside the component. Layout
authority (§3.6: layout consumes measured sizes, produces regions) is bypassed:
there is no region for the renderer to diff against, no damage rectangle to
report, and overflow degrades to silent truncation or half-painted buffers.
B2 also collapses two ARCHITECTURE.md boundaries (§3.3 component model and §3.7
renderer) into one function signature, making the renderer RFC's cell-type and
diff work unimplementable without re-breaking every component. B2 is not a
rendering contract; it is the absence of one.

**The conflation itself is the load-bearing finding:** as long as one type is
called `Frame` and sketches pass it as `&mut` to user code, every future reader
will re-litigate builder-vs-buffer. The resolution below eliminates the word
from the user-facing seam entirely.

## 5. Recommendation (normative proposal)

**Adopt C with A's user-facing seam.** Normative pipeline and ownership:

```text
state (&Model, &Slot) ──► describe ──► Desc (owned, user built, framework owned after return)
                                        ──► layout ──► Placed<'d> (borrows Desc, framework owned)
                                                        ──► paint ──► CellFrame (&mut, framework owned, reused)
                                                                        ──► backend instruction stream
```

Ownership contracts (proposed normative rules):

1. **State → description.** `view`/`describe` receives only shared borrows
   (`&Model`, `&Slot`, `&Context`) and returns `Result<Description, Error>`.
   No `&mut` parameter, no I/O, no interior mutation of framework state.
   Same inputs ⇒ same description (determinism already required by RFC-0002 §1).
2. **Description → layout.** Layout consumes `&Description` plus viewport and
   produces owned placements *borrowing* description text (`Placed<'d>` pattern
   proven in the prototype). Layout performs no text copying and no backend I/O.
   Fallible (`Overflow`), with the change-class invalidation rules (RFC-0002 §5)
   deciding whether this stage even runs.
3. **Layout → frame.** `paint` is the sole holder of `&mut CellFrame`. It is
   infallible by construction (layout guarantees bounds) and operates on a
   **reused double buffer** owned by the renderer (§3.7). Component code can
   neither name nor touch this type.
4. **Frame → backend.** The renderer diffs new vs. previous buffer and emits the
   instruction stream. Unchanged buffers emit nothing (quiescent cost O(1)-ish
   per RFC-0002 F2.6 layering).
5. **Error rule.** Any `Err` in describe/layout aborts the pipeline *before*
   paint; the previous buffer is retained and the diff emits nothing. There is
   no partial-output state anywhere in the contract — the property B1/B2 both
   fail (measured §3).
6. **Naming rule.** The user-facing type is called `View` or `Description`
   ( bikeshed in implementation RFC); the identifier `Frame` is reserved for
   the internal cell buffer and **MUST NOT appear in the `Component` trait**.
   This ends the builder-vs-buffer conflation by construction.

What this preserves from RFC-0002: §§1–2 (loop, identity/keys/slots), §3 state
classes and "no lifetimes escape", §4 dispatch, §5 change classes (they gate
stages 2–4: `none` skips all, `repaint` skips layout, `layout` runs all), §6
lifecycle, §7 ergonomics, §8 F2.1–F2.6 end-states — C *is* F2.1 implemented,
with the internal `&mut` confined to §3.7 where ARCHITECTURE.md already puts it.

## 6. RFC-0002 patch list (proposed — NOT applied; RFC owner edits)

1. **§8 F2.1, paragraph 1:** append — "`&mut Frame` (or any mutable render
   surface) MUST NOT appear in the `Component` trait. The sketches' `frame:
   &mut Frame` parameter is superseded; see new §8.x."
2. **New §8.x "Render pipeline end-state (resolves sketch/text contradiction)":**
   adopt the §5 pipeline above (rules 1–6) verbatim as normative, including the
   `Frame`-naming reservation.
3. **All five API sketches (§"API sketches"):** rewrite `fn view(model, frame:
   &mut Frame)` to `fn view(model: &Model, ctx: &Context) -> Result<View,
   Error>` form (returning owned nodes; builder-style `column![…]` helpers
   construct `View` values, not push into frames). Mark sketches as normative
   *shape* (return-owned-description) while keeping names non-normative.
4. **§3 "No lifetimes escape" bullet:** extend with the partial-output rule —
   "a failed `view` yields no description; there is no partially-built frame in
   user-visible state" — citing this resolution's B1/B2 measurements.
5. **§5 `repaint` bullet:** add the pipeline-gating sentence — "`repaint`
   reuses the previous layout (skips stage 2); `none` skips stages 1–4" — so
   change classes visibly drive the C pipeline.
6. **Open question (new, for Discussion):** whether `describe` and `layout`
   may fuse for `repaint`-only subtrees (placement-`Vec` pooling) — declared
   an implementation detail, not a contract change.
7. **Amendment history:** append entry recording this resolution's adoption
   (date, author, experiment path).

## 7. Migration impact

- **RFC-0002 itself:** sketch-only rewrite plus one new subsection; no contract
  in §§1–7 changes meaning (the normative text already describes A/C — only
  the sketches described B). Estimated diff: small.
- **ARCHITECTURE.md:** no change required. C respects every §3 boundary
  (component §3.3 produces descriptions; renderer §3.7 owns buffers; layout
  §3.6 owns regions; core stays TTY-free §3.10).
- **API_DESIGN.md:** no change required. C satisfies §4 purity
  (descriptions are values; golden tests assert `View` equality without a
  terminal) and §1 intent-over-mechanics (zero user cell math).
- **Future code:** unblocks the renderer RFC (cell type, diff, damage) and the
  implementation RFC (exact `View`/`Description` type, `dyn`-compatible seam
  per F2.2) — both were stalled on which side of `&mut Frame` they live on.
  Answer: the framework side.
- **Constraints compliance:** prototypes are std-only (zero deps), edition
  2024, stable 1.98.1, TTY-free, no globals in proposed design (the counting
  allocator is harness-only), no `Rc<RefCell>`, no nightly, MIT context.

## 8. Remaining uncertainties (open, with proposed owners)

1. **Exact `Description` encoding** (struct-of-nodes vs. command list vs.
   retained handles for damage): deliberately NOT decided here — needs the
   renderer RFC's cell-type and damage-region decisions first. Owner: renderer
   RFC author. Risk if decided here: premature lock-in (API stability question J).
2. **`Placed` lifetime vs. pooled `Vec` ergonomics** in real layout code with
   constraint solving: prototype layout is a linear stacker; a Taffy-backed
   solver may prefer owned placements. Either is contract-compatible (both are
   framework-internal). Owner: implementation RFC.
3. **Whether `view` should return `Change` alongside `View`** (piggybacked
   invalidation hint) or keep `update`'s return as the sole source: prototype
   punts (update returns `Change` per RFC-0002 §5). Owner: implementation RFC.
4. **`Outcome::Observed` survival** (RFC-0002 OQ1): untouched by this
   resolution; no evidence either way from these prototypes. Owner: RFC-0002
   Discussion week.
5. **Microbenchmark limits:** figures in §3 are prototype-relative
   (single allocator counter, release profile, one machine) — suitable for
   ordering candidates, NOT for publishing as framework performance claims.
   Real budgets belong to benches per ARCHITECTURE.md §8.11.

## 9. Reproduction

```powershell
cd D:\jilva\jilva-rs\experiments\rfc-0002\rendering-model
cargo run --release   # expect: 19/19 PASS, RESULT: ALL CHECKS PASS, exit 0
```

Toolchain: `rustc 1.98.1`, edition 2024, no dependencies (offline-clean).
`target/` is gitignored inside the experiment dir; no production paths
(`crates/`, `examples/`, `tests/`) were touched — verify with
`git status --short` (only `docs/research/` + `experiments/` entries expected).

## 10. Handoff summary

- **Recommended model:** C — owned `view(&self, ctx) -> Result<View, Error>` at
  the user seam; framework-owned `layout` (borrowing) + internal reused
  `&mut` cell buffer; `Frame` reserved for the internal buffer, banned from
  the `Component` trait.
- **Evidence:** 19/19 compile-tested checks + measured allocations (§3,
  experiment dir). Strongest discriminators: B1's measured orphan nodes and
  B2's half-painted buffers vs. C's no-partial-output rule; paint at ~20
  B/render proving buffer reuse.
- **RFC-2 patch list:** §6 above (7 items, sketch rewrite + one new
  subsection; no §§1–7 semantic change) **as amended by §11 deltas R1–R4.**
- **Open uncertainties:** §8 above (5 items, all with proposed owners).

## 11. Cross-review (Phase 1.6 — vs. COMPONENT_FEASIBILITY.md)

Reviewed: `docs/research/COMPONENT_FEASIBILITY.md` (Subagent 2) in full, plus
`heterogeneous-children/exp01-dyn-seam/src/lib.rs` (render seam, tests 1–8),
`exp03-slots-erasure/src/lib.rs` §§`ErasedSlot`/`ChildDesc`/`SlotTable`
(lines 36–165 + `reconcile`), and `threading/exp04-send-policy/src/lib.rs` in
full. Question under review: can the proven component system actually PRODUCE
the §5 recommended descriptions (owned `View` from `&self` + `ctx`)?

### 11.1 Verdict

**Compatible — no load-bearing contradiction.** The split seam, the slot table,
and Send-at-boundary all plug into the §5 pipeline without changing its
ownership contracts. One genuine correction (Send position, R1), one scoping
clarification (boxing rule, R3), and one prototype caveat (R4) below; the rest
is agreement.

### 11.2 Agreements (with pointers)

1. **Owned-return render seam.** exp01's `Render::render(&self, ctx: &Ctx)
   -> String` (exp01 `lib.rs:29-35`) IS the §5 user seam at scalar scale:
   shared borrows in, owned data out, no lifetime escapes — the object-safety
   comment there ("return type is owned … so no `&self`-borrowed lifetime
   escapes") is the same rule as my §5 rule 1. Test 7
   (`render_borrows_nothing_across_frames`, exp01 `lib.rs:290-297`) proves
   frames retained across renders — the exact property my checks #4–#5 assert
   for tree-shaped `View`. A tree of owned `Node`s composes identically to
   `column`'s owned join (exp01 `lib.rs:123-129). → COMPONENT_FEASIBILITY §A.1.
2. **Split seam required; single trait impossible.** The E0308/E0191/E0038
   chain (COMPONENT_FEASIBILITY §A.1, negative proof in-repo) independently
   forces what §5 assumes: `describe` lives on the object-safe render half,
   `update` on the `Sized`-quarantined channel. My patch item 1 (ban `&mut`
   from the trait) and Agent 2's patch P1 (name the split seam) are the same
   edit from opposite sides — they must land together (see R2).
3. **No-clone / no-arena / mount-once boxing preserved.** exp01: `Box<dyn
   Render>` = 16B stack + one heap alloc per child *at construction*, render
   borrows `&[Box<dyn Render>]` (§A.2, `size_of` asserts). exp03 alloc-map:
   64 mounts → 64 boxes, then +0 over 200 steady reconciles (§A.6). Nothing
   clones the model (update is `&mut` in place, §A.5/`SharedState::update`);
   no arena anywhere. My description allocation (caller-owned, O(visible
   rows)) is the same cost category exp03 explicitly names ("per-frame
   descriptions … caller-owned O(children), bounded … distinct from slots").
   → COMPONENT_FEASIBILITY §A.5–A.6.
4. **`'static` bound on erased children is compatible.** `Box<dyn Render +
   Send>` needs `T: 'static` (E0310, §A.1). This rules out components
   borrowing model data across frames — which §5 rule 1 already forbids
   ("No lifetimes escape"). The bound bites nothing the pipeline allows.
5. **Slot table does not break pipeline ownership.** exp03's table owns
   `Box<dyn ErasedSlot>`; `reconcile` consumes caller-owned `Vec<ChildDesc>`
   and hands out generation-guarded `SlotId`s; the sole `&mut`
   (`as_any_mut`, exp03 `lib.rs:47-54`) is update-only — matching my rule 1
   ("single legal mutation site") verbatim. Stale-handle → `None` and
   drop-with-cleanup feed my rule 5 as `layout`-class work, no new error
   kind. No `unsafe`, no stored borrows. → COMPONENT_FEASIBILITY §A.5.
6. **Send-at-boundary does not conflict with view purity.** Purity constrains
   I/O and mutation, never thread-transfer; policy (b) (no `Send`/`Sync` on
   models/slots, `Send` only on bridge job/completion types —
   COMPONENT_FEASIBILITY Part C, exp04 `lib.rs:66-83`) is strictly more
   permissive than any universal bound, so everything provable under
   require-`Send` still holds. The `Render + Send` opt-in seam (exp01
   `lib.rs:43-46`) and opaque `Effect::Task` routing (exp04 test 5) fit §5
   unchanged: effects stay data, execution stays above core.
7. **Sugar discipline matches.** exp02's `column!` → tested macro-free
   expansion (§A.4) is the same "macros strictly additive" rule my patch
   item 3 relies on for the rewritten sketches.

### 11.3 Contradictions / corrections

- **C1 (genuine correction — my doc's Send posture).** My §7 carried
  RFC-0002's contested "require `Send`" line without challenge, and my §6
  patch list has no Send item. exp04's E0277 pair (universal-`Send` rejects
  `Rc<RefCell>` models AND `Rc` slot memos — the RFC-0002 §3
  "observably-pure memo" shape) plus the sufficient bridge (test 4, `!Send`
  state resting on the loop thread) overturns it. **Position change: this
  track now endorses policy (b).** → delta R1.
- **C2 (no contradiction — keys).** Part B's required-by-contract table
  (runtime flags, not type-level) governs *which state survives*; my pipeline
  governs *what survives it* (descriptions → layout → paint). The
  `positional_dynamic` / `mixed_keys_fallback` flags surface as ordinary
  `layout`-class work in my rule 5 — no new change class, consistent with my
  §6 item 5. Ownership of the §2 rewrite stays with Agent 2 (their P3); I add
  only the pipeline-gating pointer. → delta R3.
- **C3 (scoping clarification — "no per-frame boxing").** RFC-0002 §8's rule
  as worded could be read to ban ALL per-frame allocation, which BOTH tracks'
  prototypes violate by design: exp03 boxes one `FnOnce` init closure per
  `ChildDesc` per frame (exp03 `lib.rs:88-110`), mine owns `String`s per
  frame. The rule must mean heterogeneity boxing, not description data —
  otherwise neither track's evidence is admissible. → delta R4.
- **C4 (prototype caveat, not a contract issue).** exp03's per-frame init
  boxing is scaffolding: a real reconciler builds the init closure lazily,
  only on actual mount. If the implementation RFC copies exp03 literally, it
  reintroduces per-frame boxing through the back door. Flagged so the
  implementation RFC (and Agent 2's P7 regression baseline "64/64/+0") scope
  the +0 claim to table-side boxing, with desc-side init counted under the
  caller-owned description budget both tracks share.

### 11.4 RFC-2 patch-list deltas (amend §6; RFC owner still applies)

- **R1 (NEW, P1 — Send policy).** Change RFC-0002 §7 "`Model: Send` is
  required" to policy (b): no `Send`/`Sync` bounds on models or slots;
  `Send` only on async-bridge job/completion types. Resolves RFC-0002 open
  Q3 (recommending the `!Send`-tolerant side of the contest). Cite exp04
  E0277 pair + bridge test (COMPONENT_FEASIBILITY Part C).
- **R2 (AMEND my §6 item 3 — sketch rewrite).** Rewritten sketches must show
  BOTH halves of the agreed seam: owned-return `view` (this track) AND
  explicit-erasure children (`Vec<Box<dyn Render>>` element type named, per
  Agent 2 P1). Either half alone re-invites the contradiction. Joint
  acceptance criterion with Agent 2's P1/P2.
- **R3 (POINTER, owned by Agent 2 P3 — keys).** No duplicate item: adopt
  Part B's table into RFC-0002 §2, plus one pipeline-gating sentence in §5
  (misuse flags surface as `layout`-class work; no fourth change class).
- **R4 (NEW, P2 — boxing-rule scope).** Amend §8 F2.2's allocation rule to:
  "no per-frame trait-object boxing for child identity (mount-once,
  measured); per-frame description allocation is caller-owned, bounded by
  visible children, and budgeted separately." Cite exp03 alloc-map + my §3
  table. Requires lazy init construction (C4) in the implementation RFC.
