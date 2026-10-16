# RFC-2 Final Technical Review — Component API & Rust Architecture (Phase 1.7)

- Reviewer: Subagent 2 — Component API & Rust Architecture Reviewer
- Date: 2026-10-16 | Branch: `phase1.7-rfc-review` (read-only; no commits)
- Under review: `docs/RFC/0002-component-model.md` (Phase 1.6 amended state, 723 lines)
- Read: RFC-2 (full) · `docs/research/RENDERING_MODEL_RESOLUTION.md` (§5 + §11) ·
  `docs/research/COMPONENT_FEASIBILITY.md` (+ cross-review §Gaps) · `docs/ARCHITECTURE.md`
- Constraints honoured: MIT-only, no production code changes, no new deps, all
  sketches treated as PROPOSED-NOT-IMPLEMENTED. No edits to RFC-2 were made;
  everything below is a proposal for the orchestrator/RFC owner to apply.

## 0. Verdict

**Conditionally accept.** Phase 1.6 closed the two load-bearing contradictions
(owned-return pipeline normative; Send policy (b) adopted). What remains is
**six precise inconsistencies** (I1–I6, all with file:line), none requiring a
design re-think — each has an exact amendment below. **Two are acceptance
blockers** (I1 stale loop text, I3 Outcome-vs-Change return split); the rest
can land as editorial follow-ups but should land before the implementation RFC.

Acceptance blockers: I1, I3. Maintainer decisions required: D1, D2, D3 (§7).

---

## 1. Assessment per charge

### (A) Rendering contract — PASS with two stale-text remnants (I1)

The normative/sketch contradiction identified in RENDERING_MODEL_RESOLUTION §1
is **resolved at the normative level**: §8 F2.1 bans `&mut Frame` from the
trait (`0002-component-model.md:343-346`), the six-stage pipeline
(describe→reconcile→layout→paint→diff→commit) is normative
(`:402-418`), the naming rule reserves `Frame` for the internal cell buffer
(`:417-418`), and all five sketches use the owned-return form
(`fn view(..) -> Result<View, Error>`, `:443`, `:462`, `:483`, `:493`).
Grep for `&mut Frame` returns exactly 3 hits, all ban/supersession language —
**zero sketch or trait occurrences remain**. Component-facing vs internal vs
backend layering matches ARCHITECTURE.md §§3.3/3.6/3.7/3.5: user code builds
`View` data (`:394-396` "no component contract names char-indexed cells";
`:411-412` "user code performs no cell math"); the framework owns layout,
`&mut CellFrame`, diff; the backend receives the instruction stream (`:105-112`
of the resolution doc, adopted at `:402-418`).

Two remnants contradict this (§6 I1): the §1 loop diagram still labels the
change class `(A: state/layout/paint)` (`:54`) — a pre-R1 taxonomy that
matches neither `none/repaint/layout` nor anything else in the doc; and §1
rule 2 says "`view` receives only shared references (`&Model`, `&Slot`) **and
a layout region**" (`:66-67`), which re-admits geometry into the user seam
that §8 rule 3 (`:411-412`) and the cell-model dependency (`:394-400`)
exclude. Both are stale text, not design positions — fix by deletion
(amendments A1, A2).

Fail-clean coverage is complete: failed `describe` retains previous
`Desc` + key set with zero mounts/drops (`:407-410`, joint G1 rule) and
"a failed `view` yields no description" (`:153-155`). No partial-output state
exists in the contract.

### (B) Change taxonomy — PASS with one naming-bridge gap (I2, minor)

§5 adopts RFC-1 Q13 verbatim as `none` / `repaint` / `layout` (`:213`), states
the no-fourth-class rule with the RFC-1 amendment tripwire (`:214-219`),
defines pipeline gating per class (`:230-231`, `:420-421`), the max-over-batch
rule (`:246-247`), over/under-reporting discipline with debug damage
assertions (`:247-252`), and the RFC-3 theme/profile `repaint`-only plumbing
(`:232-238`). Batching coherence holds: §1 rule 4 (one view+diff per source
event, overflow cap → `Error::UpdateOverflow`, `:73-76`) composes with §5
(`none` across a batch ⇒ no frame, `:221-223`) without contradiction.

Remaining gaps, all minor:
1. **Concept↔variant bridge unstated.** Taxonomy is lowercase (`repaint`)
   but the sole code sketch emits `Change::Repaint` (`:460`) with the comment
   "RFC-1 Q13 name: state-only, no re-layout" — "state-only" is the wrong
   gloss (should be "paint-only") and no sentence maps lowercase concepts to
   PascalCase variants. Trivially fixable (amendment B1).
2. **Theme-bump batching.** §5 says version mismatch "re-resolves attributes
   and repaints" (`:236-238`) — correct class, but the §1-rule-4/§5-max
   batching rule (`:246-247`) takes the max over *update returns only*.
   A theme bump on an otherwise-`none` batch must still force a `repaint`
   frame; the max rule as written would skip it. One sentence fixes it
   (amendment B2).
3. **OQ numbering slip (editorial).** Open questions list reads "4. … 6. …
   5." (`:636-642`) — renumber, no semantic content.

### (C) Heterogeneity — PASS on mechanism, sketch shows only half the seam (I3-blocker-adjacent, I4)

Normative mechanism is sound and evidence-backed: object-safe render half +
erased slot channel (split seam forced by the E0308/E0191/E0038 chain,
FEASIBILITY §A.1); `Vec<Box<dyn Render>>`-shaped objects behind a
homogeneous fast-path enum with boxing fallback (`:348-357`); generational
slot table, hand-rolled, zero-dep (`:359-368`); mount-once boxing with the
64/64/+0-over-200 baseline (`:422-425`); key/collision/reorder contract table
(`:95-106`); stale-handle→`None`, type-change⇒replacement (`:103-104`).
Object safety (no generics/`Sized` on render path, `:349-350`), lifetimes
(owned descriptions, no borrow escapes, `:151-155`), allocation timing
(mount-once slots vs caller-owned per-frame `Desc` O(visible), `:353-356` +
`:422-425`) are all stated. The R1–R4/R2-C1 deltas from both cross-reviews
are absorbed: Send (b) (`:321-325`, `:375-377`), keys required-by-contract at
runtime (`:86-91`, `:100-102`), boxing-rule scoping (slot boxing vs desc
data, `:353-356` + `:422-425`), reconcile ordering with fail-clean slots
(`:407-410`).

What is missing is **joint acceptance criterion R2** (RESOLUTION §11.4 R2):
sketches must show BOTH halves — owned-return `view` AND explicit-erasure
children with the `Box<dyn Render>` element type named. The composition
sketch (`:493-500`) shows only homogeneous `View::column(vec![…])` with the
shape note claiming "erasure inside the owned-return form — one combined
mechanism, not two" (`:502-505`). That note directly contradicts the §8 F2.2
normative text (two mechanisms: `dyn` seam + enum fast path) and FEASIBILITY
P1 (split seam must be named). A reader implementing from the sketch alone
builds an enum/Vec<View> world with no `dyn Render` in it — the same
sketch-vs-text trap Phase 1.6 just closed on `Frame`, re-opened on erasure.
Amendment C1 pins the sketch; amendment C2 reconciles the shape note.

Companion inconsistency: **`update`'s return type disagrees between
sketches** — counter returns `Change` (`:455-461`) while the `Component`
trait sketch returns `Outcome` (`:476`). §4 defines `Outcome::Consumed /
Ignored / Observed / RequestFocus` (consumption + focus) and §5 defines
`Change` (invalidation) — orthogonal axes, but no sentence says whether
`update` returns one, the other, or a pair, and the trait sketch names only
`Outcome`. This is the second acceptance blocker (amendment C3), because the
implementation RFC cannot type `update` until it is answered.

### (D) Send — HOLDS (no creep found)

Policy (b) is stated identically in three places — §3 ownership mechanics
(`:142-146`), §7 threading (`:321-325`), §8 F2.4 (`:375-377`) — with
evidence cites (E0277, exp04) and the `Rc`/`RefCell` ergonomics rationale.
OQ3 is marked RESOLVED (`:632-635`) and the handoff marks the `Send` contest
resolved pending new evidence (`:697-699`). Handoff text contains no
"require `Send`" remnant (the C1 correction from RESOLUTION §11.3 was
applied). `Effect::Task` is opaque routed data executed above core
(`:314-319`), consistent with FEASIBILITY Part C test 5. **No universal
`Send`/`Sync` creep anywhere.** One optional hardening: `Effect::Task`'s
`Send` requirement on the job type is implied ("bridge job/completion types")
but never written on the `Effect` definition itself — amendment D1 makes it
explicit so a future reader cannot infer `Task: Send`-free core execution.

### (E) Effects — data-only containment HOLDS; lifecycle RFC question list delivered (§8)

`update` is synchronous, core names no thread/executor/spawn (`:313-319`);
`Effect::After`/`Effect::Task` are declared data executed by the opt-in
bridge (`:314-316`); the overflow cap bounds follow-up frames (`:73-76`,
OQ4 `:636-637`). Containment is airtight at the contract level. §8 of this
review lists what the future lifecycle/effects RFC MUST answer (ownership,
cancellation, delivery, errors, scheduling, backpressure, runtime boundary).

---

## 2. Confirmed improvements since Phase 1.6 (evidence refs)

1. `&mut Frame` fully excised from trait + sketches (3 grep hits, all ban
   language) — RESOLUTION §6 items 1–3 applied (`:343-346`, `:402-418`,
   sketches `:436-500`).
2. Six-stage pipeline normative with per-stage ownership + `Frame` naming
   reservation — RESOLUTION §6 item 2 applied (`:402-425`).
3. Keys required-by-contract at runtime with observable flags, not type-level
   — FEASIBILITY Part B applied (`:86-102`).
4. Send policy (b) in §§3/7/8 + OQ3 resolved — RESOLUTION R1 applied
   (`:142-146`, `:321-325`, `:375-377`, `:632-635`).
5. Generational slot table named, zero-dep, no-unsafe — FEASIBILITY P2/P5
   applied (`:359-368`).
6. `Effect::Task` constrained to opaque routed data — FEASIBILITY P2/P6
   applied (`:314-319`).
7. Allocation baseline 64/64/+0 recorded — FEASIBILITY P3/P7 applied
   (`:422-425`).
8. Reconcile rule (failed describe retains Desc + key set) — joint G1
   applied (`:407-410`).
9. `none` promoted to first-class return with deviation note — in-scope,
   flagged (`:221-226`).
10. Boxing-rule scoping (mount-once slot boxing vs caller-owned desc data) —
    R4/C3-C4 applied (`:353-356`, `:422-425`).

---

## 3. Remaining inconsistencies (normative impact ordered)

| ID | Location | Contradiction | Severity |
|---|---|---|---|
| I1a | `:54` vs `:213` | Loop diagram labels change class "(A: state/layout/paint)" — a taxonomy that exists nowhere else; normative §5 is `none/repaint/layout` | **Blocker (stale normative label)** |
| I1b | `:66-67` vs `:411-412`, `:394-396` | §1 r.2 gives `view` "a layout region"; pipeline says user code does no cell math, regions are framework-produced | **Blocker (user-seam geometry leak)** |
| I2 | `:460` vs `:227` | Sketch `Change::Repaint` + "state-only" gloss vs `repaint` "paint-only" concept; no concept↔variant map | Editorial (naming bridge) |
| I3 | `:455` vs `:476`, §§4–5 | `update -> Change` vs `update -> Outcome`; no joint return specified | **Blocker (unimplementable signature)** |
| I4 | `:491-505` vs `:348-357` + R2 | Composition sketch + shape note ("one combined mechanism") show only homogeneous `View`; §8 F2.2 normatively requires `dyn` seam + fast path | Must-fix (joint R2 criterion) |
| I5 | `:86-91` vs `:363-364` | §2: `Key: Clone + Eq`; §8 F2.3: `Clone + Eq + Hash` — `Hash` appears only in the latter | Editorial (bound drift) |
| I6 | `:266` vs `:475` | §6: `init() -> S` infallible; trait sketch: `fn init() -> Result<Self::State>` fallible (§6 text itself says "fallible via Result") | Editorial (signature drift) |
| I7 | `:246-247` vs `:236-238` | Max-over-batch covers update returns only; theme/profile-bump `repaint` on an otherwise-`none` batch has no batching rule | Should-fix (frame-skip hazard) |

---

## 4. Exact amendment proposals (orchestrator: apply as one edit batch)

**A1 — `:54` diagram label.** Replace `(A: state/layout/paint)` with
`(none / repaint / layout — §5)`. One-line, no other diagram change.

**A2 — `:66-67` view inputs.** Replace "and a layout region." with "plus
explicit `&Context` (theme tokens, focus path, capability profile, tick
clock value). `view` receives no regions, rectangles, or cell coordinates;
all geometry is framework-produced (§8 pipeline rules 3–4)." This aligns §1
with §§3/8 and closes the geometry leak.

**B1 — concept↔variant bridge + gloss fix (`:460` + §5 head).** (i) In §5
after `:213`, append: "In Rust these surface as `Change::None`,
`Change::Repaint`, `Change::Layout` (lowercase names are the contract
concepts; PascalCase names are their enum variants)." (ii) In `:460` change
comment to `// RFC-1 Q13 name: paint-only, no re-layout`.

**B2 — theme-bump batching (`:246-247`).** Append: "Version-driven work
(THEME/profile bumps, §5 `repaint`-only) joins the batch maximum as an
implicit `repaint` vote: it can lift a batch from `none` to `repaint` but
never to `layout`." Prevents the otherwise-`none` frame skip.

**C1 — composition sketch shows both seam halves (`:491-500`).** Replace the
parent-child sketch with a form that names erasure at the child boundary and
owned nodes at the return boundary, e.g. children held as the erased render
seam and resolved to owned `View` nodes at return (exact type names
non-normative; the `Box<dyn Render>`-shaped element type MUST appear in the
sketch or its immediately-adjacent annotation). Mark the homogeneous
`View::column(vec![…])` construction as the homogeneous fast-path spelling,
not the general mechanism.

**C2 — shape-note correction (`:502-505`).** Replace "one combined
mechanism, not two" with: "Two mechanisms at distinct boundaries: erased
render objects (`dyn`-compatible seam, §8 F2.2) at the child-holding
boundary, homogeneous owned `View` nodes at the return boundary. The
homogeneous fast-path enum avoids boxing for the common set; mixed lists
fall back to boxing (mount-once rule, §8 F2.2)."

**C3 — `update` return (BLOCKER — §§4–5 + `:476`).** Add one normative
paragraph (suggested home: end of §5, before `:253`): "`update` reports both
axes: consumption (`Outcome`) and invalidation (`Change`). The exact carrier
(whether `update` returns a `Change`, an `(Outcome, Change)` pair, or
`Outcome` carrying a `Change` field) is fixed by the implementation RFC, but
both values are required outputs of every `update` — a sketch returning only
one axis is illustrative shorthand, not the signature." Update the `:455`
and `:476` sketches with a `// returns BOTH Outcome + Change — carrier fixed
by impl RFC` annotation so neither sketch is mistaken for the signature.

**C4 — Key bounds (`:86-91`).** Change "`Clone + Eq` value" to
"`Clone + Eq + Hash` value" (matching `:363-364`), or state explicitly that
`Hash` is required only of the framework's sibling-level map key projection.
Prefer the former (single bound, no projection machinery).

**C5 — `init` signature (`:266`).** Change "`init() -> S`: construct default
slot state. Pure, fallible via `Result`" to "`init() -> Result<S, Error>`:
construct default slot state. Pure." — matching the `:475` sketch and the
"fallible" claim already in the sentence.

**D1 — `Effect::Task` Send bound (`:314-319`).** Append: "`Effect::Task`'s
job closure and its completion value are `Send`-bounded at the bridge
(policy (b), §7); core routes the effect without executing, spawning, or
naming threads." Makes the implied bound auditable.

**E1 — OQ renumber (`:636-642`).** Renumber "4, 6, 5" → sequential; no
content change.

---

## 5. Evidence refs (all verified this session)

- `&mut Frame` grep over `docs/RFC/`: 3 hits, all in
  `0002-component-model.md:343,345,716` (ban/supersession/amendment-history)
  — no trait/sketch occurrence.
- Owned-return sketches: `:443`, `:462`, `:483`, `:493` (`-> Result<View,
  Error>`, `&`-only inputs).
- Pipeline rules 1–6: `:405-418`; fail-clean slots: `:407-410`; no-lifetimes:
  `:151-155`.
- Change taxonomy + gating + max rule: `:211-252`, `:420-421`.
- Heterogeneity seam: `:348-357`; slot table: `:359-368`; identity table:
  `:95-106`; allocation baseline: `:422-425`.
- Send (b) ×3: `:142-146`, `:321-325`, `:375-377`; OQ3 resolved: `:632-635`.
- Effects data-only: `:313-319`; batching cap: `:73-76`.
- Cross-review deltas confirmed absorbed: RESOLUTION §11.4 R1–R4
  (R2 joint sketch criterion EXCEPTED — see I4); FEASIBILITY cross-review
  G1 (ordering/fail-clean), G2 (desc-vs-slot allocation scoping), G3
  (hint-channel coordination, carried to impl RFC).

---

## 6. Acceptance blockers

1. **I1 (stale loop text):** `:54` taxonomy label and `:66-67` layout-region
   input contradict the normative pipeline. Apply A1 + A2.
2. **I3 (update return split):** `Change` vs `Outcome` sketches disagree and
   no joint carrier is specified. Apply C3 (plus C1/C2 for the R2 sketch
   criterion, which the Discussion week will otherwise re-litigate).

No production code, dependency, or architecture-boundary changes are needed
for any item above — all are RFC-text edits.

---

## 7. Maintainer decisions (need a ruling in Discussion week)

- **D1 — `Outcome::Observed` survival.** RFC-2 leans drop (`:625-627`,
  `:699`); this review concurs (binary consumption + `RequestFocus` cover
  the evidenced cases; analytics-style observation can ride `Message`
  fan-out). Recommend DROP unless a widget author tables a concrete design
  needing it. (Carried OQ1.)
- **D2 — Keys advisory vs type-enforced.** Endorse the RFC's current
  required-by-contract/runtime-flags position (FEASIBILITY Part B rationale
  stands: the type system catches none of the four misuse classes
  completely). Recommend HOLD; reopen only with a widget-author prototype
  showing compile-time keys catching the unstable-across-frames case.
  (Carried OQ2.)
- **D3 — `describe`/`layout` fusion + `View`-piggybacked change hints.**
  Endorse "implementation detail, not a contract change" (`:638-641`) with
  the G3 coordination note: if `describe` ever returns per-subtree hints,
  Part-B misuse flags ride the same channel — the implementation RFC owns
  that call. No RFC-2 text change beyond C3's carrier paragraph.

---

## 8. What the future lifecycle/effects RFC MUST answer (charge E)

Containment (`Effect` as opaque routed data, core thread/executor-blind) is
sufficient as a boundary but specifies no runtime. The lifecycle RFC must
normatively answer, at minimum:

1. **Ownership:** who owns a pending effect (loop queue? bridge handle?) from
   enqueue to delivery; what owns it across `update`→`view`→diff within one
   batch.
2. **Cancellation:** what cancels an effect (unmount of the emitting slot?
   newer effect superseding? explicit token?); is `cleanup` guaranteed to
   observe outstanding effects of its slot, and in what order.
3. **Delivery:** exactly-once vs at-least-once into `update` as `Message`;
   ordering between timer completions, task completions, and child-emitted
   messages within one batch drain (FIFO per §1 r.3 — state whether effect
   completions share that FIFO or a prioritised lane).
4. **Errors:** effect-construction errors vs effect-execution failures vs
   completion-handler errors — which surface as facade `Error`, which as
   `Message`, which are log-only; path context requirements (§6 parity).
5. **Scheduling:** what "After(dur, msg)" means without a clock in core
   (who holds the timer wheel — bridge or lifecycle?); tick-subscription vs
   timer-effect interaction; coalescing of duplicate timers.
6. **Backpressure / overflow:** the OQ4 cap value methodology; per-source vs
   global accounting (update-enqueued vs effect-produced); error vs
   silent-truncation-with-warning semantics; observability (counter? flag?).
7. **Runtime boundary:** the bridge interface shape (spawn/join/apply split
   per exp04) declared as the ONLY path effects execute; alpha scope ruling
   (timers-only vs `Task` included) with the seam unchanged either way;
   `Send` bounds written on the bridge types, not inferred.

Until that RFC lands, §7's "Execution policy lives above core with the
bridge" is correctly a non-goal pointer, not a gap — but the pointer must
name this question list (suggest citing it from OQ4).

---

## 9. Handoff (orchestrator)

- **File delivered:** this review (exclusive file, new — no other files
  touched, no commits).
- **Apply path:** amendments A1–A2, B1–B2, C1–C5, D1, E1 are RFC-owner edits
  to `docs/RFC/0002-component-model.md`; none touch production code, deps, or
  sibling RFCs. Suggested order: blockers first (A1, A2, C3), then R2 sketch
  pair (C1, C2), then bridges (B1, B2, C4, C5, D1, E1).
- **Joint criterion status:** RESOLUTION R2 sketch rule is the only
  cross-review delta NOT yet satisfied — C1/C2 close it. All other R1–R4 and
  G1–G3 items verified absorbed.
- **Recall:** available for follow-ups — sketch rewording, `update`-carrier
  options analysis, lifecycle-RFC question refinement, or Discussion-week
  maintainer-brief preparation.
