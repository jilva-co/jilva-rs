# RFC Decision Tracker — Jilva Phase 2 (acceptances recorded)

## Current decision — 2026-10-17 continuation

The project owner supplied the Phase-2 implementation directive and asked to
continue the existing project on 2026-10-17 (Asia/Tashkent). That directive
explicitly authorizes maintainer-driven acceptance without an external wait.
This is today's authorization; the 2026-10-16 entries below are preserved as
previously written records, not newly verified historical approvals.

Decision: **RFC-0001/0002/0003 remain accepted for Phase-2 continuation**.
The existing review documents were inspected today; their presence and content
are evidence, but their claimed authorship/independence and historical compiler
runs were not independently reproduced. Historical GitHub statements below
remain dated records; the current platform state was checked separately.
No community or external maintainer approval is inferred.

Current GitHub verification by the orchestrator (2026-10-17, `gh api graphql`):
main is `30ea9077c19f0d2b0c0bdb129216a6eab3ae7ede`; Discussions #4/#5/#6
have the recorded creation timestamps and each has `comments.totalCount = 0`
at today's check. PRs #1/#2/#7 report merged at 2026-10-16T15:36:06Z,
16:16:44Z and 16:37:17Z respectively. These are current API facts, not proof
that the old acceptance entries had independent authors or external approval.

The technical gate for backend implementation is satisfied by the recorded
seam decisions below. Component, renderer and theme implementation are outside
this authorization. The consolidated acceptance checklist records current
source evidence and deferred phase requirements separately.

### Backend seam clarifications approved for this continuation

- Preserve `Backend::{name, enter, leave, size, poll, read, submit, flush,
  capabilities}` and the existing `io::Result` signatures. Core owns shared
  events, commands and profiles; terminal depends on core; core stays free of
  terminal libraries and ambient environment/TTY access.
- `poll` reports source readiness within the requested wait. `read` may filter
  unsupported source events and return `None`; readiness cannot promise a
  mapped framework event. Scheduling/syscall overhead is outside the timeout
  bound. Decoder I/O errors propagate; only explicitly unsupported decoded
  events are safely skipped and counted.
- `submit` validates the entire batch before appending and performs no I/O.
  `flush` preserves order and reports errors, but OS/terminal writes cannot
  be rolled back. Pending output is discarded after a flush attempt to avoid
  replaying a prefix already written; the caller must recover explicitly.
- `io::Error` is public at the backend boundary. Core `Error` is the shared
  aggregate for a future runtime, not the only error exposed by every public
  API. No undocumented automatic `Error::Backend` conversion is promised.
- Capability snapshots are cached for a session; dynamic refresh queues and
  frame-boundary application remain lifecycle work (Phase 5). Phase 2 handles
  resize data without implementing the runtime/frame transaction.
- Terminal focus notifications are distinct from component focus synthesis;
  additive terminal-focus/paste events are permitted. Key press/repeat/release
  information must be preserved when supplied by Crossterm. Event kind alone
  is not proof of an enhanced keyboard encoding.

Current source checks: RFC-2 §8 names owned views, erased children and
generational slots; §5 requires both consumption and change outputs, leaving
the exact Rust carrier to the implementation RFC. RFC-3 §5 records domain
consumption and §4 records capability-independent merge; Q7 records the frame
commit invariant. Existing reviews are `RFC_DESIGN_REVIEW.md`,
`RFC2_FINAL_TECHNICAL_REVIEW.md`, `RFC3_TERMINAL_READINESS.md` and
`CROSS_RFC_ACCEPTANCE_REVIEW.md`. Current verification does not rerun their
prototype experiments or imply that the future implementations already exist.

The historical statements below retain their original dates and wording.
Where they conflict with this current record, this 2026-10-17 decision and the
updated acceptance checklist govern continuation.

- Owner: Subagent 1 — Architecture & RFC Governance Engineer (Jilva Phase 2)
- Last updated: 2026-10-16 (Phase 2: RFC-0001/0002/0003 ACCEPTED under maintainer-driven approval)
- Rule: decisions are recorded here by a maintainer with dated rationale, per
  `docs/RFC/README.md` (decision-authority amendment 2026-10-16). This file
  is the authoritative decision record; RFC headers mirror it.
- Prior state: the Phase 1.7 CASE-A record (awaiting Discussion, earliest
  decisions 2026-10-23) is superseded by the governance amendment below. It
  is kept for audit, not deleted (§"Phase 1.7 → Phase 2 transition").

## Governance amendment (2026-10-16 — external wait waived)

- Authorization: project owner, 2026-10-16. The external 7-day Discussion
  wait is WAIVED; development proceeds under maintainer-driven approval
  (AI implementation + independent audit, external input welcome but not
  blocking). Recorded in `docs/RFC/README.md` (decision-authority section
  with evidence requirements); that section governs from here on.
- Discussion threads #4/#5/#6 (opened 2026-10-16T16:17:19Z / 16:17:50Z /
  16:18:16Z, 0 comments at acceptance) remain historical fact: recorded,
  never backdated, no external approval claimed. The §"Decision timing"
  earliest-decision timestamps (2026-10-23) are hereby retired as gating
  constraints; they are preserved below as history.
- Acceptance evidence standard applied (per README amendment): independent
  review coverage + verified amendment landing + triaged open questions +
  dated rationale — all four met for each RFC (see per-RFC entries).

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
- Status today: **DECIDED 2026-10-16** — all three RFCs accepted under
  maintainer-driven approval (see per-RFC entries). The pre-amendment
  "no decision before earliest-decision timestamps" rule is retired by the
  governance amendment above; the timestamps below are preserved as history.

## Decision timing (1-week minimum per `docs/RFC/README.md`)

Earliest eligible decision = opening + 168h. Computed, not backdated:

| RFC | Discussion | Opened (UTC) | Earliest decision (UTC) |
|---|---|---|---|
| RFC-0001 | #4 | 2026-10-16T16:17:19Z | **2026-10-23T16:17:19Z** |
| RFC-0002 | #5 | 2026-10-16T16:17:50Z | **2026-10-23T16:17:50Z** |
| RFC-0003 | #6 | 2026-10-16T16:18:16Z | **2026-10-23T16:18:16Z** |

No Accept / Reject / Postpone before the corresponding timestamp, no exceptions.

> 2026-10-16 update: this timing gate is RETIRED by the governance
> amendment (external wait waived by project owner). Timestamps preserved
> as history; decisions recorded below rely on the waiver, not on elapsed time.

## Per-RFC tracking

### RFC-0001 — Framework Scope, Principles, Non-Goals

- Status: **Accepted** (2026-10-16; header + amendment history updated in
  `docs/RFC/0001-framework-scope.md`).
- Discussion thread: **open, historical** — https://github.com/jilva-co/jilva-rs/discussions/4
  (Ideas, opened 2026-10-16T16:17:19Z UTC, 0 comments at acceptance).
  No external approval claimed.
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
  [2026-10-16: ALL CLEARED — (1) waived by governance amendment; (2) Q7
  confirmed consistent on careful read (§3.9 rules + §4.1 internal types
  re-exported via facade; future `jilva-error` split not precluded);
  (3) accepted — Q10/Q11 closed by RFC-2 §8 F2.2/F2.3, Q12 scheduled ≤
  Phase 3 as a tracked exit obligation, Q13/Q14/Q3/Q1/Q2 owned by
  lifecycle/layout/renderer RFCs; (4) thread silent, nothing raised.]
- Maintainer decision: **ACCEPT (2026-10-16)**
- Rationale: 14 scope questions answered; Must/Should/Could/Out matrix
  enforceable with mechanical gates (matrix notes i–iii); change taxonomy
  resolved by RFC-2 adoption; permanent-vs-deferred split verified correct
  (cross-RFC review §B); §8 mapping complete (14/14 owned/scoped/deferred);
  audit found no status misrepresentation and Discussion-readiness YES.
  Residuals are owned elsewhere, not scope evasions (see deferred list).

### RFC-0002 — Component Model, State Ownership, Event Flow

- Status: **Accepted** (2026-10-16; header + amendment history updated in
  `docs/RFC/0002-component-model.md`; residual audit F2 fixed in the
  acceptance entry).
- Discussion thread: **open, historical** — https://github.com/jilva-co/jilva-rs/discussions/5
  (Ideas, opened 2026-10-16T16:17:50Z UTC, 0 comments at acceptance).
  No external approval claimed.
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
  [2026-10-16: CLEARED under waiver — (1) waived; (2) risk accepted and
  carried as a standing trigger (key semantics are amendment-gated
  post-acceptance; future widget-author evidence reopens via amendment, never
  silently); (3) DECIDED — `Outcome::Observed` DROPPED, keys HOLD at
  runtime-required (reviewer D1/D2 concur with author); (4) accepted —
  deferred to implementers/lifecycle RFC as still-open items below.]
- Maintainer decision: **ACCEPT (2026-10-16)**
- Rationale: hybrid justified by decision matrix (only option clearing
  flow + testability + reuse + determinism); F2.1–F2.6 end-states normative
  with compiler evidence; both acceptance blockers (I1 stale loop text, I3
  joint Outcome/Change carrier) verified landed, residual F1/F2 editorial
  items fixed (F1 pre-acceptance, F2 in the acceptance entry); contested
  items decided with reopen triggers rather than left ambiguous.

### RFC-0003 — Semantic Styling, Theme Tokens, Capability Fallbacks

- Status: **Accepted** (2026-10-16; header + amendment history updated in
  `docs/RFC/0003-theming-architecture.md`).
- Discussion thread: **open, historical** — https://github.com/jilva-co/jilva-rs/discussions/6
  (Ideas, opened 2026-10-16T16:18:16Z UTC, 0 comments at acceptance).
  No external approval claimed.
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
  [2026-10-16: CLEARED under waiver — (1) waived; (2) DECIDED — Q3 HOLDS
  (Text + debug marker), Q9 recipe-first, Q2 documentary-only, Q1/Q5
  deferred with evidence requirements as still-open items below;
  (3) triggers acknowledged and carried as standing amendment triggers (§
  "Standing triggers" below).]
- Maintainer decision: **ACCEPT (2026-10-16)**
- Rationale: token set justified per-token with amendment-gated evolution;
  merge/quantize/resolution rules normative and testable (9-item harness);
  P1–P3 wording repairs verified landed; snapshot policy endorsed;
  authority splits agreed (classify vs mechanism); Unicode HOLD adopted as
  document-and-defer with a Phase-≤3 renderer/Unicode RFC requirement
  (7 required decisions) — containment without pretense.

## README / roadmap / RFC status accuracy (re-verified 2026-10-16, Phase 2)

- `docs/RFC/README.md` governance (maintainer-driven approval, evidence
  requirements, amendment history): the acceptance authority for all three
  decisions below. OK.
- All three RFC headers read `Status: Accepted (2026-10-16)` with dated
  acceptance entries + discussion links; lifecycle notes preserved as
  history. OK.
- `docs/ROADMAP.md` Phase 1 RFC boxes checked; exit criterion
  "RFC-1/2/3 are accepted" met; Phase 2 marked started (not complete). OK.
- `README.md` status block / Scope / arch overview: pre-existing
  "to be confirmed by RFC" wordings are now superseded by acceptance —
  flagged to the orchestrator as a docs follow-up OUTSIDE this tracker's
  scope (no status misrepresentation in the acceptance record itself).
- **Corrections needed from other subagents: none found** (carried from
  Phase 1.7; re-verified headers/history/discussion links this session).

## Deferred items with owners (still-open at acceptance — none blocks Phase 2)

| # | Item | Owner | Evidence gate to close |
|---|---|---|---|
| D1 | Effect-cap value + overflow semantics (error vs truncation) | Lifecycle/effects RFC author (Phase 5); implementer measures | Implementer measurement + documented cap |
| D2 | `describe`/`layout` fusion for repaint-only subtrees | Implementer (implementation detail, contract unchanged) | Implementer decision, no RFC text change |
| D3 | Default palette RGB values + taste-review process (RFC-3 Q1) | Implementation/widgets RFC (Phase 6) with contrast-test evidence | Contrast checks pass + documented taste step |
| D4 | 256 quantization metric: Euclidean RGB vs perceptual (RFC-3 Q5) | Implementation RFC (Phase 6) | Snapshot comparison evidence |
| D5 | Unicode symbol type + width/segmentation authorities + truncation rule (RFC-1 Q12, RFC-3 Q8) | First buffer/diff (renderer/Unicode) RFC, **no later than Phase 3** — tracked Phase-3 exit obligation (already in `docs/ROADMAP.md`) | Renderer RFC decision; `char`-non-entrenchment holds until then |
| D6 | Keyboard ladder depth + Kitty push/pop teardown pairing + cursor single-owner (RFC-1 Q13) | Lifecycle/event RFC (Phase 5) | Documented ladder either way |
| D7 | Tick/time-source naming (RFC-1 Q3) | Lifecycle RFC (Phase 5) | Named source + fixed-clock test seam |
| D8 | Layout remainder tie-break + min-size policy + over-constraint behaviour (RFC-1 OQ1/OQ2) | Layout RFC (Phase 4) | Named rules, snapshot-reproducible |
| D9 | Profile-refresh queue mechanics (RFC-3 Q7 residue) | Lifecycle RFC | Queue mechanics specified |

## Standing triggers (post-acceptance amendments required — no silent drift)

Per cross-RFC review A4, carried forward as binding: token renames/additions
→ amendment RFC; third built-in theme or subtree-scoped themes → scope
check; `Effect` growth beyond opaque routed data → scope amendment; fourth
change class → explicit RFC-1 amendment; key-semantics change →
amendment RFC; `char`-indexed cell API past Phase 3 → scope breach.

## Phase 1.7 → Phase 2 transition (governance note)

- The Phase 1.7 record (CASE-A: awaiting Discussion, decisions eligible no
  earlier than 2026-10-23, Phase-2 NOT AUTHORIZED) is superseded by the
  2026-10-16 governance amendment (external wait waived by project owner).
  The 1.7 verification facts stand as history: threads #4/#5/#6 opened by
  @hamroqulovv with zero comments; Discussion-readiness YES ×3;
  review layer self-consistent. What changes is authority, not facts:
  maintainer-driven approval with the four evidence requirements replaces
  the elapsed-time gate.
- Audit findings disposition at acceptance: F1 (RFC-2 gloss) fixed
  pre-acceptance; F2 (RFC-2 sketch `, Error`) fixed in the acceptance
  entry; F3 (RFC-3 history) fixed pre-acceptance; F4 (CI branch list)
  housekeeping, maintainer-owned, non-gating; F5 (silent threads) closed
  by deliberate proceed-decision under waiver, recorded here; F6
  (`char`-replacement exit obligation) already codified in ROADMAP.md
  Phase 3; F7 (alpha scope cut order) carried as a planning constraint.
- Phase 1.6 → 1.7 transition note (preserved): the Phase 1.6 record stated
  repo Discussions were disabled (`has_discussions → false`), no threads
  existed, and listed manual maintainer posting steps. That state was
  superseded in Phase 1.7 (threads #4/#5/#6 exist, `has_discussions → true`).
  Full Phase 1.6 → 1.7 audit note archived in git history (pre-Phase-2 tracker revisions).
