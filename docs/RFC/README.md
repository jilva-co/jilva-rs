# RFC process

Design before code. Every significant decision in `jilva-rs` starts as an RFC.

## Lifecycle

`Draft` → `Discussion` → `Accepted` | `Rejected` | `Postponed`

- **Draft** — open a PR adding `docs/RFC/NNNN-title.md` from the template. Number = next free.
- **Discussion** — announced in Ideas; review focuses on flaws, alternatives, prior art.
- **Decision** — a maintainer merges (accepted) or closes with reasons. Accepted RFCs may still change — amendments are new short RFCs.

## Decision authority (maintainer-driven approval; revalidated 2026-10-17)

Current authorization: on 2026-10-17 the project owner supplied the Phase-2
directive and asked the implementation to continue. That directive explicitly
waives an external waiting period and authorizes evidence-based technical
acceptance. The 2026-10-16 amendment and decisions below remain historical
records; today's review does not independently certify their authorship,
timestamps, GitHub state or prior compiler runs. Current acceptance evidence
and deferred work are recorded in `docs/reviews/RFC_ACCEPTANCE_CHECKLIST.md`
and `docs/reviews/RFC_DECISION_TRACKER.md`.

Previous policy: `Draft` → `Discussion` (minimum 1 week, external reviewer
voice expected before any decision) → `Accepted` | `Rejected` | `Postponed`.

New policy (effective 2026-10-16, authorized by project owner @hamroqulovv,
2026-10-16): the external 7-day Discussion wait is **WAIVED**. Development
proceeds under maintainer-driven approval: AI implementation plus independent
audit constitute the review record. External input is welcome but not
blocking — a maintainer may record `Accepted` / `Rejected` / `Postponed`
with dated rationale in `docs/reviews/RFC_DECISION_TRACKER.md` at any time.

Reason: the Phase 1.7 Discussion threads (#4/#5/#6, opened 2026-10-16,
zero comments) showed that a mandatory external wait gates progress on
participation that may never arrive, while the independent review layer
(technical reviews + cross-RFC review + acceptance checklist + final audit)
already provides competent adversarial scrutiny. Waiting on silence is not
review.

Evidence requirements for acceptance (unchanged in force, adapted in venue).
No RFC is accepted unless ALL hold, recorded in the tracker:

1. Independent review coverage: each RFC has a dedicated technical review
   plus cross-RFC consistency review, all recommending
   `ready-pending-discussion` or better with no unaddressed acceptance blocker.
2. Amendment landing verified: every review amendment claimed as landed is
   re-verified present by file:line or a named section (audit-trail drift, Phase 1.7 findings
   F1/F3 class, blocks acceptance until fixed).
3. Open questions triaged: each open item is either resolved with evidence in
   the acceptance entry or recorded as still-open with a named owner RFC/phase
   in the tracker. Nothing is faked as resolved.
4. Dated rationale + deferred-item list in `docs/reviews/RFC_DECISION_TRACKER.md`.

Discussions still welcome: threads remain open and any future external
comment is triaged by the maintainer (accept as amendment RFC, rebut with
evidence, or record as deferred). Historical discussion facts — thread
numbers, opening timestamps, comment counts at decision time — are recorded,
never backdated, and external approval is never claimed where none was given.

Amendment history of this policy:

- 2026-10-17: current project-owner directive revalidates the waiver and
  continuation. Review files are inspected as repository evidence; historical
  independent authorship and external activity are not inferred from them.
- Pre-2026-10-16: `Draft` → `Discussion` (min. 1 week) → decision. External wait mandatory.
- 2026-10-16: external 7-day wait waived by project owner; maintainer-driven
  approval with the four evidence requirements above; discussions remain open
  but non-blocking. First exercised: acceptance of RFC-0001/0002/0003
  (see `docs/reviews/RFC_DECISION_TRACKER.md`).

## What needs an RFC

- Public API shape, component model, theming system, MSRV, dependency policy.
- Anything a user would have to unlearn if we got it wrong.

## What does not

- Typos, wording, repo chores, prototype code.

## Rules

- One proposal per RFC. Small wins.
- Include prior art (what do Ratatui / Cursive / Bubble Tea / Textual do here?).
- Include non-goals and open questions explicitly.
- Prototype code may accompany an RFC but never substitutes for it.
