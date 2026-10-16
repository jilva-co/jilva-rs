# Cross-RFC Acceptance Review — Framework Scope & Consistency (Subagent 4, Phase 1.7)

- Scope: RFC-0001 (scope parent, as amended Round 2), RFC-0002 (component model, Phase 1.6 amendments), RFC-0003 (theming, Phase 1.6 amendments), `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`.
- Constraints honoured: MIT-only (no dependency in any proposal violates it — `palette` is MIT-or-Apache-2.0, admissible only via future RFC; `serde`/`unicode-*` posture is additive-only); no RFC file edited; amendments below are proposals to the orchestrator.
- Status of all three RFCs: **Draft**. Nothing here accepts anything. Recommendations are `ready-pending-discussion` vs `needs-work` only.

## (A) RFC-1 alpha scope: is `0.1.0` achievable?

**Verdict: yes, achievable — but at the upper bound of alpha size. The Must list is large and schedule slip is the top scope risk (RFC-1 Drawbacks (iv) names it).**

Essentials vs future separation is clean and mechanically enforced:

- Must (alpha-blocking, ROADMAP Phase 7 = Q4): run loop + restoration; flex/grid layout with remainder rule; token vocabulary + default theme + tier resolution; 6 widgets (text, list, table, input, tabs, status bar); keyboard + resize + tick + user-message; gated mouse click/scroll; in-memory test double; golden + scripted-event tests headless.
- Should (seam present at alpha, no extra public API): spanning refinements, focus scopes, `jilva-motion` seam, read-only observer seam, `jilva-cli` as facade consumer, protocol-adapter seam reservation, scrollable containers.
- Could (future RFC + prototype evidence, no seam reserved): third backend evaluation, session persist, virtualised tables, effect/timer API, theme gallery.
- The matrix Notes (i)–(iii) are the enforcement mechanism: every Must row needs a headless test; Should rows add no alpha API; Could rows are not commitments. **Phase-2 gate check: any implementation PR that ships a Could item or a Should item with public API beyond the seam fails scope review.**

Watch items on achievability (not blockers, tracked in matrix §D):

1. Six widgets + tier snapshots + golden tests is a lot for one alpha. If slip threatens, cut in this order per the RFC's own logic: tabs/status-bar richness first (they compose from text primitives), never the loop, restoration, test double, or tier matrix.
2. `Outcome::Observed` (RFC-2 §4, self-flagged non-minimal) must not leak into the layer-(a) path (RFC-1 OQ15). Lean drop stands.
3. `Effect::After/Task` is the Should-level async bridge forming early — acceptable iff core ships zero async deps and no executor types (RFC-1 OQ14 condition). Any timer/thread growth inside core triggers a scope amendment.

## (B) Non-goals: permanent vs deferred split

**Verdict: correctly split. No item needs reclassification; two wordings need tightening (proposed amendments A1, A2).**

- Permanent (revisit = superseding ADR + RFC): GUI toolkit; app-framework concerns (config/networking/logging/DI); mandated/bundled async runtime; raw-ANSI styling (no escape hatch — RFC-3 R1 hardened); ambient globals; backend-aware core; CSS-unit leakage; macro-primary APIs; `cfg(windows)` in core; 3D; sound; framebuffer/GPU. All correctly permanent — each would dissolve a load-bearing boundary (§3/ADR-0003).
- Deferred (postponed, each needs its own RFC): Q5 list (tree/charts/modal/toast/scrollbar/scrollables, spanning, focus scopes, motion, devtools, cli, images/protocols, remote optimisation, suspend/resume, clipboard). All correctly deferred — each is RFC-sized, none is load-bearing for alpha correctness.
- Correctly refused details inside deferred topics: pixel scrolling (cells only), spreadsheet engine, multi-focus, persistence/networking in framework, motion-required UI, profiling-in-production. Correct — each contradicts a principle.
- Two tightening proposals (not reclassification):
  - **A1 (proposed):** RFC-1 Q9 macro rule says "macro may only be additive sugar" — RFC-2 §7 already commits to macro-free normative core with rustdoc-rendered expansion + misuse-case error. Propose RFC-1 Discussion confirm that commitment as the acceptance bar for the implementation RFC (no RFC-1 text change by this reviewer).
  - **A2 (proposed):** RFC-1 matrix Unicode row schedules `Cell { symbol: char }` replacement "no later than Rendering Engine (Phase 3)". Correct scheduling (see §D), but propose the orchestrator track it as a **named Phase-3 exit-criterion obligation**, not a matrix note — golden tests written against `char` cells would need rewriting (RFC-1 OQ12). No permanent `char`-model dependence is permitted.

## (C) Compatibility matrix

Legend — Phase: the earliest ROADMAP phase whose exit criterion needs the rule. Evidence: the RFC section or feasibility proof that carries it.

| # | Concept | Owning RFC | Rule (one line) | Dependents | Phase | Evidence | Open conflict |
|---|---|---|---|---|---|---|---|
| 1 | Identity | RFC-2 §2 (§8 F2.3 storage) | Paths (default, static) + keys (required-by-contract for dynamic sequences, runtime-enforced); collision/mixing/type-change misuse table | Layout (keyed children stable), renderer (slot-stable damage), widgets | Phase 4 | Feasibility exp03 (64 mounts/200 reconciles/+0 boxes); `COMPONENT_FEASIBILITY.md` 29 tests | **Yes — minor:** keys advisory-vs-required at type level still contested (RFC-2 OQ2); runtime-required position holds; Discussion decides whether builders enforce at compile time |
| 2 | State ownership | RFC-2 §3 | Four classes, one owner each: app Model / framework slot map (keyed by identity) / per-frame transient / derived-in-view; children emit messages upward, never mutate siblings; shared data via explicit `&Context` | Events (routing targets), lifecycle (init/cleanup), tests (slot asserts) | Phase 4 | API_DESIGN §4 constraints satisfied; borrow story §8 F2.4 (no `unsafe`, no stored borrows) | No — R-app vs ephemeral split agreed; `Rc`/`RefCell` ergonomics preserved by Send-at-boundary policy |
| 3 | Events | RFC-2 §4 (+ lifecycle RFC for ladder depth) | Taxonomy Key/Mouse/Resize/Tick/Message/FocusGained-Lost; `#[non_exhaustive]` + `Message(M)` wrapper extensibility; total dispatch (resize broadcast top-down; key/mouse focus→ancestors→fallback; tick to subscribers; message to owner); FIFO, no reentrancy, batch cap → `Error::UpdateOverflow` | Lifecycle (read/decode), layout (resize path), focus | Phase 5 | J-list item (RFC-1 OQ4) answered; Kitty teardown pairing + ladder depth deferred to lifecycle RFC (RFC-1 OQ13) | **Yes — owned elsewhere:** keyboard ladder depth (legacy-only vs Kitty push/pop at 0.1.0) unresolved; scope requires documented ladder either way |
| 4 | Change taxonomy | RFC-1 Q13 (scope) / RFC-2 §5 (mechanism) | Three classes `none` / `repaint` / `layout`; reconstruct is identity output producing layout+repaint, not a fourth class; max-over-batch; over-report safe, under-report caught by debug damage-vs-diff asserts | Layout invalidation, theme propagation, renderer diff | Phase 4 (rule), Phase 3 (renderer proof) | **RESOLVED by adoption** — RFC-2 §5 adopts Q13 verbatim (R2-C1 retired); deviation (`none` first-class return) flagged, within scope | No — terminology mapped in RFC-3 §6; fourth-class revival requires explicit RFC-1 amendment |
| 5 | Layout invalidation | RFC-1 Q13 + Layout row (scope); layout RFC (mechanism) | `repaint` = text/attribute change in identical regions, never invalidates; `layout` = constraints/content-size/resize/width-table-relevant text change, invalidates bounded subtree, cascades stop at unchanged ancestor | Renderer (skip-layout fast path), theme (paint-only claim relies on it) | Phase 4 | ARCHITECTURE §8.9 consideration scoped; container-bounded cascades owned by layout RFC | No — but container-cascade detail still owed by the layout RFC |
| 6 | Theme resolution | RFC-3 §§1–5 (R1–R5, merge order, fallback chain) | Tokens+variants only, no raw ANSI (no escape hatch); visual attrs carry no geometry; capability resolution is pure `(Theme, Profile)` fn; merge order app-deltas→base→default, per-token last-writer-wins, capability-independent, quantization last | Change taxonomy (`repaint` claim), renderer (attribute-only churn), backend (receives resolved attrs only) | Phase 6 (schema), Phase 3 (diff must budget attribute churn) | Tier snapshots + delta-merge + mono tests (§Testing 0–8); F3.1–F3.5 aligned/resolved | **Yes — bounded:** overlay-pair snapshot scope (only `Selected+Focused` golden-filed, rest property-tested — RFC-3 OQ4 residual, escalated to widgets evidence); quantization metric (Euclidean vs perceptual — OQ5); extension-missing-key behaviour (OQ3) |
| 7 | Capabilities | RFC-3 §5 (shape + resolution); terminal RFC (detection) | Detect in `jilva-terminal`, cross seam as plain-data typed-domain profile (Color/Input/Screen + aggregate transport, per-domain consumption); conservative default Ansi16; `NO_COLOR`→Mono; Unknown background→no auto-switch | Theme resolution, input decoding, renderer/lifecycle | **Phase 2** (detection + test double must exist before renderer/lifecycle work) | `CAPABILITY_MODEL_RESOLUTION.md`; profile shape on RFC-1 J-list (additive fields only) | **Yes — cadence owned by lifecycle RFC:** snapshot-at-start + per-domain versions resolved (RFC-3 OQ7); queue mechanics still owed |
| 8 | Frame atomicity | RFC-2 §1/§4 (loop rules); lifecycle RFC (policy) | One event → one transactional update → at most one painted frame; intermediates never painted; in-flight resize frames complete-or-drop, never half-painted; `describe` failure retains Desc, zero mounts/drops, frame aborted clean | Renderer diff, resize handling, tests (bit-identical replay) | Phase 5 (lifecycle), Phase 3 (diff) | `RENDERING_MODEL_RESOLUTION.md` 19/19; pipeline rules §8 (1–6) | **Yes — detail owed:** exact drop-vs-complete policy is tested implementation detail (lifecycle RFC) |
| 9 | Errors | ARCHITECTURE §3.9 (rules); RFC-1 Q2/Q14 (taxonomy is J-list); RFC-2 §6 (component impact) | Facade error only public; internal types with inward `From`; `panic!` for violated invariants only; errors unwind through teardown (terminal always restored); component `init` fallible with path context, `cleanup` infallible | Lifecycle teardown, all boundaries (§9 cross-cutting) | Phase 2 (seam + taxonomy skeleton) through Phase 5 (restoration tests per exit path) | Wording flag RFC-1 OQ7 (§3.9 vs §4.1 "internal types re-exported" — consistent on careful read, needs reviewer confirmation) | **Yes — wording only:** confirm visibility wording so a future `jilva-error` split is not precluded; no semantic dispute |
| 10 | Lifecycle restoration | ARCHITECTURE §3.2 + lifecycle RFC (mechanism); RFC-1 matrix (Must); RFC-2 §6 (component coverage) | setup→loop→teardown; restoration on normal, `?`-error, and panic (where feasible) paths; cooperative shutdown; guarantee holds for component `update` panics too (unwind through teardown) | Errors, backend (push/pop flag pairing), tests (restoration test per exit path) | Phase 5 (guarantee), Phase 2 (backend setup/teardown edge) | ARCHITECTURE §8.7 scoped; tick-source fixed-clock seam (RFC-1 OQ3) positioned as injected value, source named by lifecycle RFC | **Yes — mechanics owed:** panic-guard mechanism, Kitty flag pairing, tick source, suspend/resume hooks (edge, not core) all owned by the lifecycle RFC |

Cross-cutting consistency notes:

- No ARCHITECTURE.md boundary violations in any sibling (RFC-1 R2-C5/R2 record; RFC-2 Handoff §"Conflict flags"; RFC-3 Alt-3 upholds ADR-0003 decision 2: backend never sees tokens).
- Shared questions all owned: A (change classes — RFC-1/RFC-2, resolved) · B (theme propagation — RFC-3 §6 version+paint, watched for scoped-themes OQ6) · D (capability entry — RFC-3 §5 + typed domains, cadence → lifecycle) · G (smallest API — RFC-1 Q14 sketch ~14 lines, preserved by RFC-2 §7 and RFC-3 §4; gap: heterogeneous mechanism now answered by RFC-2 §8 F2.2 `dyn`-seam + mount-once boxing — shared-G acknowledgement satisfied, no scope amendment needed) · I (Unicode — width/segmentation authority → layout/renderer RFCs; style width-neutral, RFC-3 OQ8) · J (expensive-to-change seven — identity, event extensibility, constraint vocab, token vocab, profile shape, error taxonomy, backend seam — all flagged load-bearing in Discussion).
- Gaps closed since Round 2: heterogeneous children (RFC-2 §8 F2.2 named mechanism — R2-C3 retired subject to Discussion acceptance); key-storage backing (hand-rolled generational table, zero-dep — R2-C6 retired subject to acceptance); `Effect`-vs-async watch continues under OQ14 conditions.

## (D) Phase-2 entry: what must be true before Phase 2 vs Phase 3

ROADMAP order: Phase 2 = Terminal Engine (backend seam, detection, test double) → Phase 3 = Rendering Engine (buffer/diff/damage) → Phase 4 = Layout & Components → Phase 5 = State & Interactivity → Phase 6 = Design System → Phase 7 = Alpha.

### Required before Phase 2 starts

1. RFC-1/2/3 through Discussion week (min. 1 week per `docs/RFC/README.md`); no implementation PR may assume contested positions (keys compile-time enforcement, overlay-pair snapshot scope, quantization metric, ladder depth) until Discussion resolves them.
2. Backend seam interface shape agreed (setup/teardown/read/write/size/capabilities — RFC-1 Q14 J-list) so Phase-2 code has a stable target; error-taxonomy skeleton (facade-only public) agreed so Phase-2 failures have somewhere to go.
3. Capability-profile domain split agreed (Color/Input/Screen + aggregate transport — RFC-3 §5 Phase-1.6 model) since Phase 2 *produces* the profile every later phase consumes.
4. `char`-cell replacement tracked as a **Phase-3 exit obligation** (A2 above): Phase-2 test-double and Phase-3 buffer work must not entrench `char`-shaped APIs; component text stays `&str`/grapheme-cluster with width-supplied offsets (RFC-2 §8).

### Required before Phase 3 (may proceed in parallel with / during Phase 2)

1. Symbol-type shape + width authority (`unicode-width`, `width` vs `width_cjk`) + segmentation authority (`unicode-segmentation`) + blank/continuation equality + truncation rule (never split grapheme/wide cell; 1-column-remainder edge) — owned by the first buffer/diff RFC (RFC-1 OQ12). **Explicitly NOT required before Phase 2:** Phase 2 needs only the scheduling commitment (replacement ≤ Phase 3) and the `char`-non-entrenchment rule above. Demanding a full Unicode renderer pre-backend would invert the ROADMAP order.
2. Diff granularity + cursor-minimisation + worst-case full-churn bound + damage→backend flow (ARCHITECTURE §8.2/§8.10, RFC-1 Q10 measurable forms) — the rendering RFC's own acceptance content, not a Phase-2 prerequisite.
3. Layout remainder tie-break + minimum-size policy + over-constraint behaviour (RFC-1 OQ1/OQ2) — needed by Phase 4, informative for Phase 3 damage bounds, not gating Phase 2.
4. Lifecycle mechanics (panic guard, Kitty pairing, tick source, drop-vs-complete, profile-refresh queue) — needed by Phase 5, not gating Phase 2 beyond the seam edge.

### Standing prohibitions (all phases)

- No permanent `char`-model dependence: any API naming `char`-indexed cells past Phase 3 is a scope breach requiring amendment, not a convenience.
- No backend-aware core types, no CSS-unit leakage, no async runtime in core, no raw-ANSI escape hatch — each is a permanent refusal (RFC-1 Q9), not a phase convenience.

## Proposed amendments to orchestrator (this reviewer edits nothing)

- **A1 — Macro-rule acceptance bar:** confirm RFC-2 §7 macro-free normative core (rustdoc-rendered expansion + misuse-case error presented in implementation RFC) as the shared-G acceptance check. No RFC text change.
- **A2 — Phase-3 cell-replacement obligation:** track "`char` replacement ≤ Phase 3; first buffer/diff RFC owns symbol type + authorities + truncation rule" as a Phase-3 exit criterion in the orchestrator's plan (source: RFC-1 Unicode matrix row + OQ12). No RFC text change.
- **A3 — Error-visibility wording check:** Discussion to confirm ARCHITECTURE §3.9/§4.1 wording (internal types, public re-export) so a future `jilva-error` split is not precluded (source: RFC-1 OQ7). No RFC text change.
- **A4 — Standing triggers carried forward:** token renames/additions → amendment RFC; third built-in theme or subtree-scoped themes → scope check; `Effect` growth beyond opaque data → scope amendment; fourth change class → explicit RFC-1 amendment. No RFC text change.

## Acceptance recommendations

| RFC | Recommendation | Basis |
|---|---|---|
| RFC-0001 (scope) | **ready-pending-discussion** | 14 questions answered; Must/Should/Could/Out matrix enforceable; change taxonomy resolved by adoption; permanent-vs-deferred split correct; §8 mapping complete (owner/scope/defer for all 14); residual items are genuinely owned elsewhere (lifecycle/layout/renderer RFCs), not scope evasions |
| RFC-0002 (component model) | **ready-pending-discussion** | Hybrid justified by decision matrix (only option clearing flow+testability+reuse+determinism); F2.1–F2.6 end-states normative with compiler-evidence; former gaps (heterogeneous seam, slot storage, Send policy, pipeline) closed with evidence; contested items (keys type-level, `Observed` survival, effect-cap value) are legitimate Discussion fodder, not acceptance blockers |
| RFC-0003 (theming) | **ready-pending-discussion** | Token set justified per-token; no-escape-hatch refusal hardened; merge/quantize/resolution rules normative and testable (9-item harness incl. delta-merge, mono S+F, NO_COLOR, Unknown-background); authority splits agreed (classify vs mechanism); residuals (OQ3/OQ4-scope/OQ5/OQ6/OQ9 + palette values) need maintainer/widget evidence, not rework |

Unresolved dependencies (none blocks Discussion; all block specific later phases as noted): keyboard ladder depth + flag pairing (lifecycle RFC, Phase 5); tick source naming (lifecycle RFC, Phase 5); remainder/min-size/over-constraint (layout RFC, Phase 4); symbol type + authorities (buffer/diff RFC, Phase 3); overlay-pair snapshot scope + quantization metric + palette values (widgets/implementation evidence, Phase 6); error-visibility wording confirmation (Discussion itself).
