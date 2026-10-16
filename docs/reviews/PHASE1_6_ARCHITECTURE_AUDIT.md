# Phase 1.6 Architecture & Quality Audit — Independent Auditor Report

- Auditor: Subagent 5 — Independent Architecture & Quality Auditor
- Date: 2026-10-16
- Repo: `D:\jilva\jilva-rs`, branch `phase1.6-architecture-resolution`, commit `25bbd62`
- Scope read: `docs/RFC/0001-framework-scope.md`, `docs/RFC/0002-component-model.md`,
  `docs/RFC/0003-theming-architecture.md` (all Draft, amended),
  `docs/research/RENDERING_MODEL_RESOLUTION.md` (+ §11 cross-review),
  `docs/research/COMPONENT_FEASIBILITY.md` (+ cross-review),
  `docs/research/CAPABILITY_MODEL_RESOLUTION.md` (+ §12),
  `docs/reviews/RFC_DECISION_TRACKER.md`, `docs/ARCHITECTURE.md`, `LICENSE`,
  plus production-tree verification (`crates/`, `Cargo.toml`, `Cargo.lock`,
  `.github/workflows/ci.yml`).
- Method: read-everything-first, then verified structural claims against the
  production tree with grep/read (no re-running of orchestrator-verified suites;
  per tasking, exit-0 results are taken as given, not re-claimed).
- No files other than this one were created or modified by this auditor
  (working tree was clean at audit start: `git status --short` empty).

## 1. Production-tree verification (independently checked)

| Claim | Result |
|---|---|
| Production workspace zero-dep | PASS. `Cargo.lock` contains exactly 3 packages (`jilva`, `jilva-core`, `jilva-terminal`), no external deps. Neither `jilva-core` nor `jilva-terminal` has a `[dependencies]` section. |
| Experiments detached | PASS. All 5 experiment crates (`rendering-model`, `exp01`, `exp02`, `exp03`, `exp04`) carry their own empty `[workspace]` and are not workspace members; production `cargo` graph unaffected. |
| MIT-only | PASS. `LICENSE` is MIT; all three member manifests use `license.workspace = true`. No dual-license headers found. |
| `forbid(unsafe_code)` | PASS in all production crates (`jilva-core/src/lib.rs`, `jilva-terminal/src/lib.rs`, `jilva/src/lib.rs`) and all experiment crates per resolution docs. No `unsafe` found in `crates/`. |
| TTY-free core | PASS. `jilva-core` imports only `std::fmt`/`std::io` (error formatting; `error.rs:11-12`) — no terminal, env, or platform imports. No `cfg(windows)` in `crates/`. |
| Acyclic deps | PASS. `jilva-core` → nothing; `jilva-terminal` → nothing workspace-internal; `jilva` → both. Matches ARCHITECTURE.md §6. |
| No globals | PASS. No `static`/`OnceLock`/`thread_local` in `crates/`. (Only `Vec<String>` log + `String` payloads, which are ordinary owned data.) |
| No escape-injection code in production | PASS. No ANSI/CSI/escape bytes, no `crossterm`/`serde`/`tokio` references anywhere in `crates/`. Style module is token vocabulary only (`style.rs:1-6` explicitly defers resolution to RFC-3). |
| Secrets / workflow perms | PASS. `ci.yml:8-11` sets `permissions: contents: read` with an explicit least-privilege comment; no secrets referenced; no publish/release jobs. |
| RFC status labels | PASS. All three RFC headers read `Status: Draft`; no `Accepted` status claim anywhere (remaining "accept*" hits are lifecycle-process prose or "acceptance bound" performance wording — see LOW-4). |
| ROADMAP accuracy | PASS. Phase 1 RFC-1/2/3 boxes unchecked, exit criterion ("RFC-1/2/3 are accepted") unmet — accurate. |
| Tracker accuracy | PASS. `RFC_DECISION_TRACKER.md` records all decisions PENDING, discussions unopened, no backdated timestamps. The org-vs-repo Discussions venue observation (lines 146-150) is honest and actionable. |

## 2. Findings

No BLOCKERs. Two HIGHs, both pre-acceptance (not discussion-blocking).

### HIGH-1 — `Cell { symbol: char }` is live production API on the entrenchment path
- Location: `crates/jilva-core/src/renderer.rs:18-20` (`pub symbol: char`), re-exported via facade (`crates/jilva/src/lib.rs:24`).
- Reasoning: every resolution agrees `char`-cells must be replaced by a
  grapheme-capable symbol type no later than Phase 3 (RFC-1 Open Q12,
  RFC-2 §8 cell-model dependency, ARCHITECTURE §8.3–§8.4). The type is
  currently public, `Copy`-adjacent simple, and already has a passing
  doc-test — i.e. exactly the shape future code will casually depend on.
  Entrenchment risk #1 from feasibility is therefore live, not theoretical.
- Fix: before any golden-output/diff test lands (Phase 3 entry), replace with
  the renderer-RFC-specified symbol type + width/segmentation authorities, or
  at minimum mark `Cell` `#[non_exhaustive]`/doc-hidden as provisional. Do not
  write new tests against `char` cells in the meantime.
- Verification: grep `symbol: char` returns nothing; renderer RFC names
  authorities; `git log` shows the replacement preceding the first buffer/diff
  golden test.

### HIGH-2 — RFC-2 `Change::Repaint` sketch name vs normative `repaint` taxonomy
- Location: `docs/RFC/0002-component-model.md:460` (`Change::Repaint`) vs §5
  normative names `none / repaint / layout` (lines 213-244) and RFC-1 Q13.
- Reasoning: sketches are the most-read part of the RFC; a reader copying the
  counter sketch gets a fourth identifier (`Change`) alongside the adopted
  lowercase change-class vocabulary. Small today, but this is J-list
  (expensive-to-change) terminology and Discussion should pin one form.
- Fix: rename sketch to the §5 vocabulary (or state once that `Change` is the
  sketched enum carrying `none/repaint/layout`). One-line edit by RFC-2 owner.
- Verification: single vocabulary string greps clean across RFC-1 §Q13,
  RFC-2 §5, RFC-3 §6.

### MEDIUM-1 — Shared-G acknowledgement for the newly-named public seam types
- Location: RFC-1 Open Q10 (lines 579-587); RFC-2 §8 F2.2.
- Reasoning: Phase 1.6 names erasure-adjacent public surface (`View`/`Description`
  nodes, `dyn Render` seam, `Boxed` fallback) that did not exist when RFC-1's
  minimal-API path (construct→run→describe→handle→style→observe) was written.
  Both cross-reviews synthesised the sketch (Rendering §11 R2 + Feasibility P1),
  but RFC-1's record still carries Q10 as an open acknowledgement item. If the
  mechanism adds public API, G needs explicit scope sign-off.
- Fix: Discussion-week maintainer minute on Q10 confirming the seam additions
  preserve the layer-(a) minimal path; close or amend accordingly.
- Verification: RFC-1 Q10 marked resolved/acknowledged in the tracker.

### MEDIUM-2 — CI never runs on this branch
- Location: `.github/workflows/ci.yml:5` (`branches: [main, phase1-foundation]`).
- Reasoning: the `phase1.6-architecture-resolution` branch (and any future
  `phase*` branch) gets no check/clippy/test/doc runs on push. The
  orchestrator-verified exit-0 claims therefore rest on local runs only for
  this branch. Process gap, not a code gap.
- Fix: add `phase1.6-architecture-resolution` (or a `phase*` glob) to the push
  branch filter, or accept local-only verification explicitly in the tracker.
- Verification: push to the branch triggers the CI jobs.

### MEDIUM-3 — `Effect::Task` containment is contract prose with no enforcement point yet
- Location: RFC-1 Open Q14 (lines 608-615); RFC-2 §7 (lines 313-319).
- Reasoning: the "opaque routed data, executed above core only" constraint is
  correctly stated in both RFCs and evidenced by exp04 test 5. But no
  production code exists to embody it, and the condition that would force a
  scope amendment ("if `Effect` grows timers/threads inside core") has no
  named reviewer or checklist home for the lifecycle/effects RFC.
- Fix: lifecycle-RFC template carries an explicit "Effect-vocabulary vs RFC-1
  Q14" review checkbox; Discussion confirms the amendment trigger wording.
- Verification: lifecycle RFC contains the checkbox; `Effect` additions cite Q14.

### LOW-1 — Token-count prose is wrong (`~20` vs 14 actual)
- Location: `docs/RFC/0003-theming-architecture.md:94` ("~20 tokens in 4 groups")
  vs the enumerated 14 (4+3+4+3) and line 152 ("~15 tokens").
- Fix: change line 94 to "~14" (or "~15"). Editorial.
- Verification: count matches prose.

### LOW-2 — Allocation baseline in RFC-2 omits the profiler label the cross-review requires
- Location: `docs/RFC/0002-component-model.md:424-425`
  ("64 mounts → 64 slot boxes, 200 steady reconciles → +0 boxes, 0 drops").
- Reasoning: both resolutions stress these numbers are profiler-relative
  (Rendering §8.5; Feasibility claim-discipline header) and the synthesised
  P7 requires profiles labelled (release bytes for descriptions vs structural
  slot counts). The RFC-2 citation carries no label, inviting future
  misquotation as a framework performance claim — exactly what RFC-1 Q10 forbids.
- Fix: append "(structural slot counts; description bytes measured separately
  under release — see Rendering §3)" or equivalent.
- Verification: baseline sentence carries both figures with profiles named.

### LOW-3 — Facade re-exports backend seam types (sanctioned, but worth one Discussion sentence)
- Location: `crates/jilva/src/lib.rs:26-28` (`Backend`, `CapabilityProfile`, `TestDoubleBackend` re-exported publicly).
- Reasoning: RFC-1 Q3 says internal-crate paths must never appear in user code;
  re-export *through the facade* is the sanctioned path (ARCHITECTURE §7 wiring
  note, RFC-1 Q3 test-double clause), so this is compliant. Flagged only because
  a strict reader could misread Q3 as forbidding even facade-mediated backend
  types at the construction edge, where ARCHITECTURE §3.5 explicitly requires them.
- Fix: none required; optionally one clarifying sentence in RFC-1 Q3 during Discussion.
- Verification: Discussion minute acknowledges the reading; no code change.

### LOW-4 — Residual wording notes (editorial, no action needed pre-Discussion)
- RFC-1:200 "acceptance bound" and RFC-3:609 "Accepted deliberately" are
  performance-prose / ordinary English, not status claims — verified not
  misleading in context. RFC-1:383 "pending acceptance" correctly marks
  RFC-2 §2 as unaccepted. No changes proposed; listed so a future
  status-lint does not false-positive on them.

## 3. The 12 critical verification questions (reconstructed from the architecture's load-bearing decisions)

The Phase 1.6 brief's §5 list is not present in-repo; the tracker owner
confirms Discussion drafts carry the contested questions instead. The 12
questions below are reconstructed to cover the same load-bearing surface, each
answered with evidence:

1. **Do the three RFCs contradict each other on change classes?** No.
   RFC-2 §5 adopts RFC-1 Q13 verbatim (none/repaint/layout); RFC-3 §6 maps the
   same three terms. Both cross-reviews confirm (R2-C1 retired).
2. **Does any `&mut Frame` remain in the proposed user seam?** No.
   RFC-2 §8 bans it normatively; all five sketches show owned-return form
   (verified lines 436-500). Residual: HIGH-2 naming only.
3. **Can heterogeneous children actually be built?** Yes, at design level:
   split seam (`dyn Render` + erased slots + enum fast path + tuples + sugar),
   each with a named place and measured evidence (E0308/E0191/E0038 chain,
   64/64/+0 ledger). Residual: MEDIUM-1 (G acknowledgement).
4. **Is `Send` policy consistent everywhere?** Yes. Policy (b) —
   no `Send`/`Sync` on models/slots, `Send` only on bridge types — appears
   identically in RFC-2 §§7–8 and both resolutions, with E0277 negative proofs.
5. **Does capability detection stay out of core?** Yes by design:
   detection in `jilva-terminal`, plain-data domains in core, consumption rule
   (domain structs only outside two seams). Production tree has no detection
   code yet to violate this; the rule is review-enforceable.
6. **Do theme/profile switches preserve the repaint-only guarantee?** Yes,
   modulo the §12.1 clarification (describe still runs; layout skipped),
   which RFC-3's amendment history records as applied.
7. **Is frame atomicity decided compatibly with fail-clean buffers?** Yes:
   snapshot-at-start + abort-before-paint compose over disjoint events (§12.2);
   combined frame-commit invariant named with lifecycle-RFC ownership.
8. **Is the core TTY-free with backend separation intact?** Yes — verified
   against production code (§1 table), not just prose.
9. **Is production zero-dependency with experiments detached?** Yes — verified
   against `Cargo.lock` (3 packages) and all 5 experiment manifests (§1).
10. **Are allocation claims honestly scoped?** Yes in the resolutions
    (profiles labelled, publishability disclaimed); one leak into RFC-2
    prose (LOW-2).
11. **Do docs claim anything already accepted?** No — all Draft, tracker
    PENDING, roadmap boxes unchecked, discussions verifiably unopened (§1).
12. **Is there a secret/permission/dependency risk in CI or deps?** No —
    least-privilege read-only CI, no secrets, no external deps (§1).
    Residual: MEDIUM-2 (branch filter).

## 4. Top risks (for handoff)

1. **`char`-cell entrenchment (HIGH-1).** The only finding with a clock on it:
   every week of Phase-3-adjacent work written against `Cell::new('x')`
   raises the cost of the mandated replacement.
2. **Effect-bridge drift (MEDIUM-3).** The async-scope boundary is currently
   protected by prose alone; the lifecycle RFC is where it will be tested.
3. **Unrun Discussion period.** All technical resolutions are explicitly
   provisional (Draft, unaccepted, evidence-linked). The load-bearing
   contesteds — keys advisory-vs-required, `Outcome::Observed` survival,
   snapshot scope for non-load-bearing pairs, quantization metric — have
   named positions but no maintainer decisions, by design.

## 5. Readiness verdict + Phase-2 authorization

- **Verdict: DISCUSSION-READY, NOT ACCEPTED.** The Phase 1.6 work is coherent,
  honestly evidenced, and safe to put before reviewers: no load-bearing
  contradiction survives across the three resolutions and their cross-reviews,
  and the production tree matches every structural claim I could check
  independently. The two HIGHs and three MEDIUMs are pre-acceptance or
  process items; none prevents meaningful Discussion.
- **Phase-2 authorization: NOT AUTHORIZED.** RFC-1, RFC-2, and RFC-3 all remain
  **Draft**; the minimum 1-week Discussion period per `docs/RFC/README.md` has
  not run (no threads opened — verified in the tracker); no maintainer
  decision is recorded. Phase 2 (terminal engine implementation) must not
  begin until at least the capability-relevant RFC decisions it depends on
  (RFC-1 scope, RFC-3 profile shape, RFC-2 pipeline ownership) are accepted
  through that process.

## 6. Handoff summary

- Verdict: DISCUSSION-READY, NOT ACCEPTED. Phase 2: NOT AUTHORIZED.
- Counts: 0 BLOCKER · 2 HIGH (pre-acceptance) · 3 MEDIUM · 4 LOW (1 informational).
- Top risks: (1) `char`-cell entrenchment before Phase 3; (2) Effect-bridge
  drift without an enforcement point; (3) Discussion period unrun — all
  positions provisional.
- Files touched: this audit only. No commits, pushes, or branch switches performed.
