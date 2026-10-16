# [RFC-0001 Discussion] Framework scope, alpha essentials, and non-goals

> Status of the proposal under discussion: **Draft**
> (`docs/RFC/0001-framework-scope.md`, 2026-10-16, Round-2 revision).
> Per `docs/RFC/README.md`, no decision before 7 full days from this thread's
> posting timestamp. Nothing here is accepted.

## Problem

Every TUI project re-litigates the same scope fights (image support? own async
runtime? raw-ANSI styling?) per PR until the API is unlearnable. This RFC draws
the boundary once: `jilva-rs` is a **terminal UI framework** that owns the
run-loop contract — not a widget library, not a GUI toolkit, not an app
framework — with a Must/Should/Could/Out classification matrix, an
alpha-essential scope for `0.1.0`, and permanent refusals.

## Options considered

1. **Ratatui-style scope** (renderer + widgets, user owns the loop) — smallest
   API, maximal control; but restoration, dispatch order, and degradation become
   per-app folklore. Rejected: "correct underneath" cannot be delivered by a
   library that never sees the loop.
2. **Textual-style scope** (CSS + async core + devtools in v1) — designer
   participation, modern workloads; but a second language with cascade bugs in
   a cell medium, and a forced runtime choice contradicting ARCHITECTURE.md
   §2.4. Rejected for alpha; separable post-alpha RFCs possible.
3. **Single-crate scope** — rejected by ADR-0002 (recorded because reviewers
   keep proposing it).
4. **Proposed: framework scope** (Q1–Q14 + matrix) — owns loop, invalidation
   contract, token model, backend seam; apps own domain state, persistence,
   networking, recovery policy, runtime choice.

## Consequences

Owning the loop makes incremental adoption inside Ratatui/Cursive apps harder;
tokens-only styling frustrates raw-escape power users; sync-core pushes async
complexity onto users until the bridge RFC; the Must list is large for an alpha
(slip risk — mitigate by postponing Should/Could aggressively, never promoting
under pressure).

## Recommendation going in (provisional — maintainer decides after Discussion)

Accept the Q1–Q14 scope, the classification matrix, and the Q9 permanent
refusals as the bounding box for RFC-2/3 and engine RFCs.

## Evidence

- `docs/research/ARCHITECTURE_FEASIBILITY.md` F1.1–F1.5 (all incorporated:
  closed layout vocabulary, `char`-cell replacement scheduled ≤ Phase 3,
  macro-primary refusal, hello-world brevity bar ~8–15 lines, §8 mapping).
- Round-2 cross-review: R2-C1 resolved (RFC-2 adopts none/repaint/layout);
  R2-C2 agreed (single focus path); R2-C5 agreed (theming coherence).
- Hello-world sketch in RFC-1 §6 (~14 lines, facade-only) meets the brevity bar.

## Specific questions for reviewers

1. Alpha widget set (text, list, table, input, tabs, status bar) — anything
   missing that makes `0.1.0` not-Jilva, or anything to demote to Should?
2. Q13 change classes (none/repaint/layout, reconstruct as identity output) —
   any case needing a fourth class?
3. Q10 measurable performance *forms* (frame budget at 3 sizes, bounded hot-path
   allocation, no quadratic on wide screens, damage bound) — right forms?
   (Values belong to engine RFCs, not this thread.)
4. Hetrogeneous children (Open Q10): if RFC-2's mechanism adds public API, does
   the shared-G acknowledgement rule satisfy you?
5. Async scope row vs RFC-2 data-only `Effect` (Open Q14): acceptable iff core
   ships no executor types and no async deps?
6. Keyboard ladder depth for `0.1.0` (Open Q13): legacy-only or Kitty push/pop
   included, with teardown pairing?
7. ARCHITECTURE.md §3.9/§4.1 error-visibility wording (Open Q7): confirm the
   "internal types, public re-export" reading?
8. **Cell-symbol scheduling + dependency wording (new Phase-1.6 evidence):**
   confirm the `char`-cell replacement lands no later than Phase 3 — the
   first buffer/diff RFC owns the symbol-type shape, width/segmentation
   authorities, blank/continuation semantics, and the never-split-a-grapheme
   truncation rule — so no `char`-shaped API entrenches via component text
   APIs first (component side traffics `&str`/grapheme clusters only)?
   Companion wording caution: serde-exclusion must be justified by
   compile-time/MSRV-surface cost, never as a "core takes zero external deps"
   precedent, so it cannot be read as blocking the future width/segmentation
   dependencies. Confirm both?
