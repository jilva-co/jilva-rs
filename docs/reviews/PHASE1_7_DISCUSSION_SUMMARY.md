# Phase 1.7 Discussion Summary — RFC Governance & Community Review

- Owner: Subagent 1 — RFC Governance & Community Review Lead (Phase 1.7)
- Date: 2026-10-16
- Explicit statement: **CASE A (awaiting discussion) — prepare, never accept.**
  All three RFCs are under Discussion with zero comments. No decision of any
  kind is eligible before 2026-10-23, and none is recorded or recommended here.

## Timeline table

| Discussion | URL | Opened (UTC) | Author | Comments | Earliest decision (UTC) |
|---|---|---|---|---|---|
| #4 — RFC-0001 Framework Scope | https://github.com/jilva-co/jilva-rs/discussions/4 | 2026-10-16T16:17:19Z | @hamroqulovv | 0 | 2026-10-23T16:17:19Z |
| #5 — RFC-0002 Component Model | https://github.com/jilva-co/jilva-rs/discussions/5 | 2026-10-16T16:17:50Z | @hamroqulovv | 0 | 2026-10-23T16:17:50Z |
| #6 — RFC-0003 Theming Architecture | https://github.com/jilva-co/jilva-rs/discussions/6 | 2026-10-16T16:18:16Z | @hamroqulovv | 0 | 2026-10-23T16:18:16Z |

Verification method: `gh discussion view <4|5|6> --repo jilva-co/jilva-rs
--json number,title,createdAt,comments,url` — each returned
`"comments": {"nodes": [], "totalCount": 0}`. Category confirmed as Ideas
(`gh discussion view 4 --json author,category,createdAt`).

## Feedback classification

**No community feedback exists.** All three threads have zero comments at
verification time. This is stated plainly: there is nothing to classify,
summarize, or respond to. No feedback is paraphrased or manufactured in this
file; the "proposed follow-ups" below are maintainer-side prompts drafted in
a vacuum, clearly labeled as such, and **none has been posted**.

## Outstanding questions (from the RFCs' own Open-questions sections)

- **RFC-0001** (16 items, `docs/RFC/0001-framework-scope.md` §"Open
  questions"): remainder tie-break (Q1); minimum-size policy (Q2);
  tick/time-source (Q3); event-taxonomy extensibility (Q4); focus scopes
  (Q5); image fallback rule (Q6); error-visibility wording (Q7); snapshot
  location + CI ownership (Q8); perf sizes (Q9); heterogeneous-children
  mechanism (Q10, RFC-2-owned); key-storage backing (Q11); Cell-replacement
  details (Q12, Phase-3 scheduling challenged); keyboard ladder depth (Q13);
  `Effect` vs. async scope watch (Q14); `Outcome::Observed` survival (Q15);
  theming watch items (Q16).
- **RFC-0002** (unresolved only, `docs/RFC/0002-component-model.md` §"Open
  questions"; R1–R3 resolved, `Send` resolved to policy (b)):
  `Outcome::Observed` survival (leaning drop); keys advisory vs. required;
  effect-cap value/overflow semantics; describe/layout fusion (implementer
  detail). Standing conflict flags: none (renderer RFC reassigning
  cluster-splitting or cell-type duties amends this RFC).
- **RFC-0003** (10 items, `docs/RFC/0003-theming-architecture.md` §"Open
  questions"; Q4 single-winner resolved to stacking, Q7 refresh resolved to
  snapshot-at-start, Q10 `blink` resolved to refuse): palette values + taste
  review (Q1); `ThemeKind` behaviour (Q2); missing-key behaviour (Q3); Q4
  residual snapshot scope; quantization metric (Q5); scoped themes (Q6,
  not proposed); Unicode truncation authority (Q8); high-contrast built-in
  vs. recipe (Q9).

## Proposed follow-up comments (DRAFTS — do NOT post)

Nothing below has been posted to any thread. These are concise, answerable
prompts a maintainer may use to seed Discussion if threads stay silent. Each
targets a load-bearing question from that RFC's own open-questions list.

### Thread #4 (RFC-0001) — draft prompt A

> **Question for reviewers (scope Q2 + Q9):** what should `jilva-rs` 0.1.0 do
> when the terminal is smaller than the minimum useful size — scroll,
> truncate, or refuse with a clean error plus terminal restoration? And are
> the provisional perf-candidate sizes (80×24, 120×40, 200×60) the right
> ones to fix in the rendering RFC, or should they change? Concrete answers
> with a terminal size you actually use would help most.

### Thread #4 (RFC-0001) — draft prompt B

> **Question for reviewers (scope Q7):** ARCHITECTURE.md §3.9 says only the
> facade error is public API while §4.1 describes core error taxonomy types
> re-exported via the facade. The RFC reads these as consistent (internal
> types, public re-export) — do you agree, or should the visibility wording
> be tightened now so a future crate split is not accidentally precluded?

### Thread #5 (RFC-0002) — draft prompt A

> **Question for reviewers (component-model Unresolved-2):** should keys be
> *required by the type system* for dynamic lists, or *advisory with a
> runtime fallback* (current position)? Trade-off is misuse safety vs.
> beginner friction. If you have built list UIs in another framework, which
> side did you land on and what broke?

### Thread #5 (RFC-0002) — draft prompt B

> **Question for reviewers (component-model Unresolved-1):** the author
> leans toward dropping `Outcome::Observed` (non-consuming observation)
> unless a widget design evidences need for it. Can anyone show a widget
> that genuinely needs observe-without-consume, or should it go?

### Thread #6 (RFC-0003) — draft prompt A

> **Question for reviewers (theming Q3):** when app code references an
> extension token that is not defined, should the framework fall back to a
> `Text`-style render plus a debug marker, or fail loud (e.g. impossible-to-
> miss magenta in dev)? Which failure mode would you rather debug at 2am?

### Thread #6 (RFC-0003) — draft prompt B

> **Question for reviewers (theming Q5):** for mapping theme colors onto
> 256-color terminals, is nearest-RGB good enough, or worth the extra
> (zero-dependency, hand-rolled) math for a perceptual distance? Side-by-side
> snapshots of the same theme under both mappings would settle this.

## Maintainer actions checklist

- [ ] Confirm the three threads (#4, #5, #6) are in the intended venue and
  visible to reviewers (repo-level Ideas; links in the timeline table).
- [ ] Optionally seed discussion with at most the drafted prompts above —
  only if threads stay silent; never fabricate reviewer voices.
- [ ] Monitor threads through the Discussion week; record substantive
  reviewer questions into `docs/reviews/RFC_DECISION_TRACKER.md` per-RFC
  sections (tracker update only, no RFC edits without owner coordination).
- [ ] Enforce the 1-week minimum: no Accept / Reject / Postpone before
  2026-10-23T16:17:19Z (#4) / 16:17:50Z (#5) / 16:18:16Z (#6).
- [ ] After the week: record each decision + rationale in the tracker, then
  merge/close per `docs/RFC/README.md`. The tracker itself never marks
  Accepted.
- [ ] What's blocking each acceptance today: the Discussion week itself
  (CASE A), plus the per-RFC items in the tracker's "What's blocking
  acceptance" rows — notably RFC-1 Q7 wording confirmation, RFC-2
  widget-author review of identity/key semantics, and RFC-3 reviewer
  positions on Q1/Q3/Q5/Q9.
