# RFC Decision Tracker — Jilva Phase 1.7

- Owner: Subagent 1 — RFC Governance & Community Review Lead (Phase 1.7 RFC review)
- Last updated: 2026-10-16 (Phase 1.7: threads verified open, CASE A — awaiting discussion)
- Rule: all three RFCs stay **Draft** until a maintainer records a decision here
  AND merges/closes per `docs/RFC/README.md`. This file never marks Accepted.
- Prior state: Phase 1.6 record (repo Discussions disabled, no threads) is
  superseded — see §"Phase 1.6 → 1.7 transition" below. It is kept for audit,
  not deleted.

## Discussions platform state (verified 2026-10-16, Phase 1.7)

Verification method: `gh discussion view <N> --repo jilva-co/jilva-rs
--json number,title,createdAt,comments,url` for N = 4, 5, 6; plus
`gh api repos/jilva-co/jilva-rs --jq .has_discussions` → `true`.

| Discussion | URL | Opened (UTC) | Author | Category | Comments |
|---|---|---|---|---|---|
| #4 — RFC-0001 Framework Scope | https://github.com/jilva-co/jilva-rs/discussions/4 | 2026-10-16T16:17:19Z | @hamroqulovv | Ideas | 0 (`totalCount: 0`, `nodes: []`) |
| #5 — RFC-0002 Component Model | https://github.com/jilva-co/jilva-rs/discussions/5 | 2026-10-16T16:17:50Z | @hamroqulovv | Ideas | 0 (`totalCount: 0`, `nodes: []`) |
| #6 — RFC-0003 Theming Architecture | https://github.com/jilva-co/jilva-rs/discussions/6 | 2026-10-16T16:18:16Z | @hamroqulovv | Ideas | 0 (`totalCount: 0`, `nodes: []`) |

- Venue: repo-level Discussions (`jilva-co/jilva-rs`), category **Ideas** —
  matches `docs/RFC/README.md` ("announced in Ideas").
- Feedback classification: **no community feedback exists.** Zero comments on
  all three threads at verification time. Nothing is paraphrased, inferred,
  or manufactured here.
- Status today: **CASE A (awaiting discussion)** — prepare, never accept.
  No decision of any kind may be recorded before the earliest-decision times.

## Decision timing (1-week minimum per `docs/RFC/README.md`)

Earliest eligible decision = opening + 168h. Computed, not backdated:

| RFC | Discussion | Opened (UTC) | Earliest decision (UTC) |
|---|---|---|---|
| RFC-0001 | #4 | 2026-10-16T16:17:19Z | **2026-10-23T16:17:19Z** |
| RFC-0002 | #5 | 2026-10-16T16:17:50Z | **2026-10-23T16:17:50Z** |
| RFC-0003 | #6 | 2026-10-16T16:18:16Z | **2026-10-23T16:18:16Z** |

No Accept / Reject / Postpone before the corresponding timestamp, no exceptions.

## Per-RFC tracking

### RFC-0001 — Framework Scope, Principles, Non-Goals

- Status: **Draft** (verified in `docs/RFC/0001-framework-scope.md` header;
  lifecycle note present, nothing reads as Accepted).
- Discussion thread: **open** — https://github.com/jilva-co/jilva-rs/discussions/4
  (Ideas, opened 2026-10-16T16:17:19Z UTC, 0 comments at verification).
- Discussion start (UTC): 2026-10-16T16:17:19Z
- Earliest decision (start + 7 days): 2026-10-23T16:17:19Z
- Open questions (from the RFC's own §"Open questions", 16 items):
  Q1 fractional-remainder distribution rule; Q2 minimum-terminal-size policy;
  Q3 tick/time-source abstraction; Q4 event-taxonomy extensibility mechanism;
  Q5 focus scopes in/out of alpha-minimal; Q6 image/graphics fallback rule;
  Q7 ARCHITECTURE.md error-visibility wording (§3.9 vs §4.1);
  Q8 accessibility snapshot location + CI ownership (harness proposed by RFC-3);
  Q9 perf-candidate sizes/values; Q10 heterogeneous-children mechanism
  (owned by RFC-2, shared-G acknowledgement required if public API leaks);
  Q11 keyed-identity storage backing; Q12 `Cell`-replacement details
  (symbol shape, width/segmentation authorities, truncation rule — challenged
  deferral past Phase 3); Q13 keyboard ladder depth + cursor single-owner;
  Q14 `Effect` vocabulary vs. async scope (watch: acceptable iff core ships
  no executor types/async deps); Q15 `Outcome::Observed` survival (shared-G
  minimalism applies); Q16 theming watch items (two built-ins read as
  compatible; third built-in or scoped themes need a scope check).
- What's blocking acceptance: (1) Discussion week must elapse with zero
  shortcutting (earliest 2026-10-23T16:17:19Z); (2) reviewer confirmation of
  the Q7 visibility wording; (3) agreement that Q12's Phase-3 scheduling and
  the Q10/Q11/Q13/Q14 deferrals to implementation RFCs are acceptable;
  (4) no scope conflict raised in thread #4 that the RFC does not already flag.
- Maintainer decision: **PENDING**
- Rationale: **PENDING**

### RFC-0002 — Component Model, State Ownership, Event Flow

- Status: **Draft** (verified in `docs/RFC/0002-component-model.md` header +
  amendment history; "Status remains Draft").
- Discussion thread: **open** — https://github.com/jilva-co/jilva-rs/discussions/5
  (Ideas, opened 2026-10-16T16:17:50Z UTC, 0 comments at verification).
- Discussion start (UTC): 2026-10-16T16:17:50Z
- Earliest decision (start + 7 days): 2026-10-23T16:17:50Z
- Open questions (from the RFC's own §"Open questions"; R1–R3 resolved in
  Round 2 and not re-litigated without new evidence):
  Unresolved-1 `Outcome::Observed` survival (author leaning drop);
  Unresolved-2 keys advisory-with-fallback vs. type-system-required;
  Unresolved-3 `Send` bound — RESOLVED in Phase 1.6 to policy (b),
  Send-at-boundary-only (reopen only with new implementer evidence);
  Unresolved-4 effect-cap value and overflow semantics (needs implementer
  measurement); Unresolved-6 `describe`/`layout` fusion for repaint-only
  subtrees (implementation detail, implementer decides); plus the standing
  conflict-flag statement (none with RFC-1/RFC-3/ARCHITECTURE.md; renderer
  RFC assigning cluster-splitting or cell-type duties to components amends
  this RFC).
- What's blocking acceptance: (1) Discussion week must elapse (earliest
  2026-10-23T16:17:50Z); (2) widget-author review of identity/key misuse
  semantics (risk (a) in RFC handoff — hard-to-change API); (3) a position on
  Unresolved-1 (drop or evidence-backed keep) and Unresolved-2 (advisory vs.
  required); (4) confirmation that Unresolved-4/6 deferral to implementers
  is acceptable.
- Maintainer decision: **PENDING**
- Rationale: **PENDING**

### RFC-0003 — Semantic Styling, Theme Tokens, Capability Fallbacks

- Status: **Draft** (verified in `docs/RFC/0003-theming-architecture.md`
  header + lifecycle note; "must never read as Accepted").
- Discussion thread: **open** — https://github.com/jilva-co/jilva-rs/discussions/6
  (Ideas, opened 2026-10-16T16:18:16Z UTC, 0 comments at verification).
- Discussion start (UTC): 2026-10-16T16:18:16Z
- Earliest decision (start + 7 days): 2026-10-23T16:18:16Z
- Open questions (from the RFC's own §"Open questions", 10 items):
  Q1 default palette RGB values + whether a taste review step is needed;
  Q2 `ThemeKind` advisory-only vs. automatic behaviour;
  Q3 extension-token missing-key behaviour (Text-fallback + debug marker vs.
  fail-loud magenta); Q4 residual — snapshot coverage scope (just
  `Selected + Focused` + property tests, or more golden pairs; amendment if
  widgets RFC shows a third pair matters); Q5 quantization metric
  (Euclidean RGB vs. perceptual, needs snapshot evidence); Q6 scoped themes
  (currently NOT proposed; amendment trigger if RFC-2 needs dialog-scoped
  theming); Q7 RESOLVED — snapshot-at-start refresh (queue mechanics owned
  by lifecycle RFC); Q8 Unicode truncation authority (width/segmentation
  source owned by layout/renderer RFCs; amendment trigger if cluster-splitting
  assigned to style); Q9 high-contrast built-in vs. documented recipe
  (recipe-first suggested); Q10 RESOLVED — `blink` refused entirely.
- What's blocking acceptance: (1) Discussion week must elapse (earliest
  2026-10-23T16:18:16Z); (2) reviewer positions on Q1 (palette values +
  taste-review process), Q3 (missing-key behaviour), Q5 (metric, ideally
  with snapshot evidence), Q9 (recipe vs. built-in); (3) acknowledgement of
  the Q4-residual / Q6 / Q8 amendment triggers so later extensions follow
  the amendment path instead of silent drift.
- Maintainer decision: **PENDING**
- Rationale: **PENDING**

## README / roadmap / RFC status accuracy (verified 2026-10-16, Phase 1.7)

- `docs/RFC/README.md` lifecycle (Draft → Discussion min 1 week → decision):
  consistent with all three RFC lifecycle notes. OK.
- All three RFC headers read `Status: Draft`; none reads Accepted. OK.
- `docs/ROADMAP.md` Phase 1 lists RFC-1/2/3 as `[ ]` unchecked with exit
  criterion "RFC-1/2/3 are accepted" — accurate (criterion unmet, boxes
  unchecked). OK.
- `README.md` status block ("Phase 1 — Foundation … no engine, renderer, or
  widget set yet, and no release"), Scope section ("to be confirmed by RFC"),
  and arch overview ("decided by RFC-2 / RFC-3") — all accurate, no
  accepted-claims. OK.
- **Corrections needed from other subagents: none found.** No status
  misrepresentation detected anywhere checked.

## Phase 1.6 → 1.7 transition (audit note)

- The Phase 1.6 record stated repo Discussions were disabled
  (`has_discussions → false`, "repository has discussions disabled"), no
  threads existed, and listed manual maintainer posting steps. That state is
  superseded and the posting steps retired (not reproduced here — threads
  already exist). Phase 1.7 verification finds `has_discussions → true` and three open Ideas
  threads (#4, #5, #6) by @hamroqulovv with zero comments. The maintainer
  evidently enabled Discussions and posted the threads between the two
  verifications.
