# RFC Decision Tracker — Jilva Phase 1.6

- Owner: Subagent 4 — RFC Governance & Community Discussion Lead
- Last updated: 2026-10-16 15:54 UTC (Phase 1.6 cross-review: all 14 contested items mapped, drafts extended)
- Rule: all three RFCs stay **Draft** until a maintainer records a decision here
  AND merges/closes per `docs/RFC/README.md`. This file never marks Accepted.

## Discussions platform state (verified 2026-10-16 15:45 UTC)

- `gh api repos/jilva-co/jilva-rs --jq .has_discussions` → `false` (re-verified;
  matches the known 2026-10-16 finding).
- `gh discussion list -R jilva-co/jilva-rs` → error: "repository has
  discussions disabled".
- Existing threads: none visible — repo-level Discussions do not exist, so no
  duplicates possible. Org-level Discussions URL referenced in README
  (`https://github.com/orgs/jilva-co/discussions`) was not enumerated from this
  environment; maintainer should confirm whether org-level threads already exist
  before posting.
- Action taken: **no discussions created, nothing enabled** (out of scope for
  this role). Drafts are ready to post — see below.
- **NEVER claim a discussion opened without platform confirmation.** None is
  claimed here.

## Manual steps to open the three discussion threads (maintainer)

1. Go to `https://github.com/orgs/jilva-co/discussions` (README's published
   venue; repo-level Discussions are disabled) — or enable repo Discussions
   first (repo Settings → General → tick Discussions) and post under
   `https://github.com/jilva-co/jilva-rs/discussions` instead. Pick ONE venue
   for all three threads and record the choice below.
2. Click **New discussion** → category **Ideas** (per `docs/RFC/README.md`:
   Discussion "announced in Ideas").
3. Title format: `[RFC-000X Discussion] <title>` (exact titles in each draft).
4. Paste the full body from the corresponding file in
   `docs/reviews/discussion-drafts/` (verbatim; bodies already include
   problem/options/consequences/recommendation/evidence/questions).
5. Link each thread from its RFC file? **No** — RFC files stay untouched until
   the maintainer decides; record URLs in this tracker only.
6. Record the UTC posting timestamp per thread in this tracker at posting time
   (never backdate). **Earliest decision = posting time + 7 full days**
   (1-week minimum per `docs/RFC/README.md`). No Accept/Reject/Postpone before
   that, no exceptions.
7. After posting, check the "Discussion thread" row per RFC below.

## Per-RFC tracking

### RFC-0001 — Framework Scope, Principles, Non-Goals

- Status: **Draft** (verified in `docs/RFC/0001-framework-scope.md` header + §R2 record).
- Discussion thread: **not yet opened — manual steps above apply**.
  Draft: `docs/reviews/discussion-drafts/0001-framework-scope.md`,
  proposed title `[RFC-0001 Discussion] Framework scope, alpha essentials, and non-goals`.
- Discussion start (UTC): — (record at posting; never backdate)
- Earliest decision (start + 7 days): — (compute at posting)
- Open questions for maintainer (from RFC §"Open questions", 16 items; the
  load-bearing Discussion items): Q1 remainder rule; Q2 minimum-size policy;
  Q3 tick/time-source; Q4 event-taxonomy extensibility; Q10 heterogeneous
  children (shared-G acknowledgement); Q11 key-storage backing; Q12 cell-symbol
  details; Q13 keyboard ladder depth; Q14 `Effect` vs async scope; Q7 error
  visibility wording (§3.9 vs §4.1).
- Cross-review addition (Phase 1.6, in draft Q8): Cell-symbol scheduling —
  `char` replacement ≤ Phase 3, first buffer/diff RFC owns symbol type +
  width/segmentation authorities + truncation rule; serde-exclusion worded on
  compile-time/MSRV cost, never as a zero-external-deps precedent.
- Proposed answers: see draft §"Recommendations going in" (provisional only —
  maintainer decides after Discussion week).
- Evidence: `docs/research/ARCHITECTURE_FEASIBILITY.md` F1.1–F1.5; Round-2
  cross-review record in RFC-1 §§R2-C1–C6 (R2-C1 resolved by adoption,
  R2-C3/C4/C6 open or watch).
- Maintainer decision: **PENDING**
- Rationale: **PENDING**

### RFC-0002 — Component Model, State Ownership, Event Flow

- Status: **Draft** (verified in `docs/RFC/0002-component-model.md` header + amendment history).
- Discussion thread: **not yet opened — manual steps above apply**.
  Draft: `docs/reviews/discussion-drafts/0002-component-model.md`,
  proposed title `[RFC-0002 Discussion] Hybrid component model (MVU + retained identity)`.
- Discussion start (UTC): — (record at posting; never backdate)
- Earliest decision (start + 7 days): — (compute at posting)
- Open questions for maintainer (genuinely contested, per RFC handoff):
  (a) keys advisory-with-fallback vs required; (b) `Send` bound on slot state
  vs `!Send`-tolerant slots; (c) `Outcome::Observed` survival (author leaning
  drop); plus OQ4 effect-cap value/overflow semantics.
- Cross-review additions (Phase 1.6, in draft Q6–Q7 with evidence):
  rendering normative pick — owned `describe` + framework pipeline (C) vs
  `&mut Frame` in trait, `Frame` naming ban, no-partial-output rule,
  `View`-returns-`Change` sub-question (`RENDERING_MODEL_RESOLUTION.md`,
  19/19 checks); heterogeneity split seam — `dyn Render` + erased slots +
  enum fast path + tuples + macro sugar, mount-once allocation baseline
  (`COMPONENT_FEASIBILITY.md`, 29 tests, E0308/E0191/E0038/E0277 proofs).
  Keys Q refined to required-by-contract + runtime flags; Send Q refined to
  policy (b); effect-cap Q gains `Task`-opaque constraint.
- Proposed answers: see draft §"Recommendations going in" (provisional only).
- Evidence: feasibility F2.1–F2.6 incorporated into RFC-2 §§7–8; decision
  matrix (hybrid vs immediate/retained/Elm/signals); resolved R1–R3
  (change-class adoption, single focus path, invalidation split with RFC-3).
- Maintainer decision: **PENDING**
- Rationale: **PENDING**

### RFC-0003 — Semantic Styling, Theme Tokens, Capability Fallbacks

- Status: **Draft** (verified in `docs/RFC/0003-theming-architecture.md` header + amendment history).
- Discussion thread: **not yet opened — manual steps above apply**.
  Draft: `docs/reviews/discussion-drafts/0003-theming-architecture.md`,
  proposed title `[RFC-0003 Discussion] Semantic theming, token vocabulary, capability tiers`.
- Discussion start (UTC): — (record at posting; never backdate)
- Earliest decision (start + 7 days): — (compute at posting)
- Open questions for maintainer (10 items; load-bearing): Q1 palette values +
  taste review; Q3 missing-extension-key behaviour; Q4 residual (snapshot scope
  for non-load-bearing overlay pairs); Q5 quantization metric; Q6 scoped
  themes; Q7 profile refresh cadence (needs lifecycle-RFC agreement); Q8
  Unicode truncation authority; Q9 high-contrast built-in vs recipe; Q10
  `blink` shim vs refusal.
- Cross-review additions (Phase 1.6, in draft Q11–Q12 + Q4/Q5/Q7/Q8/Q10 updates):
  profile superset — typed-domain hybrid (`ColorProfile`/`InputProfile`/
  `ScreenProfile` + `TerminalCapabilities` aggregate, consumption rule, Ansi16
  default, `no_color` flag, `reduced_motion` placement); frame atomicity —
  snapshot-at-start + per-domain invalidation versions (resolves RFC-3 Q7
  policy; lifecycle queue mechanics only); snapshot strategy — representative
  goldens + property tests, exhaustive islands, evidence-driven graduation
  rule, mono S+F explicit; quantization wording ("documented metric" +
  16-role/256-distance asymmetry); `blink` refuse-entirely recommendation;
  extension keys excluded from tier snapshots; merge capability-independence.
  Source: `CAPABILITY_MODEL_RESOLUTION.md` (+ 13-item RFC-3 amendment list
  §11.5, applied by RFC-3 owner, not here).
- Proposed answers: see draft §"Recommendations going in" (provisional only).
- Evidence: feasibility F3.1–F3.5 (all aligned/resolved, Round-2 record);
  testing-strategy harness (tier matrix + mono distinguishability + delta-merge
  test); stacked-overlay position with `Disabled` absorbing.
- Maintainer decision: **PENDING**
- Rationale: **PENDING**

## README / roadmap / RFC status accuracy (verified 2026-10-16, fixed nothing)

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
- One observation for orchestrator (no file change made): `README.md` line 190
  points contributors to org-level Discussions
  (`https://github.com/orgs/jilva-co/discussions`) while repo-level Discussions
  are disabled. That link is currently the ONLY postable venue without a
  settings change. Recommend the maintainer either confirm org Discussions are
  open for posting or enable repo Discussions — and then use exactly one venue
  for all three RFC threads.
- **Corrections needed from other subagents: none found.** No status
  misrepresentation detected anywhere checked.
