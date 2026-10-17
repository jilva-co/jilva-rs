# RFC-0005: Layout Solver — Flex Row/Column Slice

- Status: **Draft — implemented on branch `phase4-layout-solver` (stacked on
  `phase3-rendering-engine`); awaiting a maintainer decision**
  (`docs/RFC/README.md`). Nothing here is Accepted until the maintainer records
  it in `docs/reviews/RFC_DECISION_TRACKER.md`.
- Author(s): Phase 4 implementation (Claude, for @hamroqulovv)
- Created: 2026-10-17
- Related: `docs/ARCHITECTURE.md` (§3.6, §8.9), `docs/RFC/0001-framework-scope.md`
  (Open Q1 remainder rule, Q2 minimum size, F1.1 closed vocabulary),
  `docs/RFC/0002-component-model.md` (change classes),
  `docs/adr/0003-terminal-backend-strategy.md` (Taffy behind the seam),
  `docs/RFC/0004-rendering-engine.md` (consumer of the rectangles),
  `docs/ROADMAP.md` (Phase 4)

## Summary

`jilva-core::layout` gains `Layout`: split a `Rect` into one `Rect` per
constraint along a row or column, in whole cells, with gap, alignment, a named
remainder rule, and an overflow policy that never panics. This is the **first
slice** of Phase 4. It deliberately excludes grid, the measure/arrange pass,
nesting, and invalidation.

## Motivation

RFC-1 asks for a closed, cell-quantised constraint vocabulary and a *named*
fractional-remainder tie-break "so snapshots are reproducible" (Open Q1), and
for over-constrained layouts that degrade without panicking (Phase 4 bullet).
Rectangles are also the natural unit for RFC-0004 damage: a widget's layout
rectangle is its damage rectangle. A small, pure, fully tested solver unblocks
both before the larger component contract (RFC-2) is built.

## Proposal

```rust
Layout::row()                       // or Layout::column()
    .constraints([Constraint::Percentage(30), Constraint::Fill(1)])
    .gap(1)
    .align(Align::Start)
    .split(Rect::new(0, 0, 80, 24)) // -> Vec<Rect>, one per constraint
```

### Vocabulary (closed; `#[non_exhaustive]`)

| Constraint | Meaning |
|---|---|
| `Length(n)` | exactly `n` cells |
| `Percentage(p)` | `floor(p% of the space after gaps)`; `p > 100` counts as 100 |
| `Min(n)` | at least `n`; then shares leftover with weight 1 |
| `Max(n)` | at most `n`; shares leftover with weight 1 up to the cap |
| `Fill(w)` | shares leftover with weight `w` (`0` takes nothing) |

No floats, no pixel or CSS units: nothing from a future Taffy integration can
leak, because the public type has nowhere to put it (ADR-0003).

### Rules

1. **Gaps** come off the axis first; if they do not fit they shrink uniformly to
   `axis / (items − 1)` rather than overflow.
2. **Initial sizes:** `Length`/`Percentage` fixed; `Min(n)` starts at `n`;
   `Max`/`Fill` start at 0.
3. **Leftover sharing (the remainder rule — answers RFC-1 Open Q1).** Capped
   water-filling: an item whose proportional share would reach its `Max` is
   frozen at the cap and the excess re-shared. When no cap binds, each item gets
   `floor(leftover × weight / total)`, and the remaining cells go one at a time
   to the **largest fractional part; ties go to the earlier item**. Integer
   arithmetic only (`u64` intermediates).
4. **Free space** when nothing can grow (no growable item, or all capped) is
   placed by `Align::{Start, Center, End}`; the odd cell of `Center` goes after.
5. **Overflow (answers part of RFC-1 Open Q2).** If fixed sizes and minimums
   exceed the axis, **later items yield first**, each shrinking toward zero.
   The result still has one rectangle per constraint, inside the area,
   ordered and disjoint. A zero-size rectangle is the signal "did not fit";
   components must tolerate empty regions (the buffer already clips to them).
6. **Cross axis** is the full extent of the area.

### Why these choices

- *Later items yield first* is predictable for the common "main content, then
  status/footer" shape only if the author orders items by importance; the
  alternative, proportional shrinking, makes sizes depend on every sibling and
  hides which item broke. Predictability is the stated goal.
- *Percentage of space after gaps* keeps `Percentage(50), Percentage(50)` always
  fitting; percentage-of-axis would overflow by the gap.
- *Largest-remainder, earlier wins* is stable under snapshot testing and keeps
  the extra cell on the leading side, where reading starts.
- `Min` and `Max` are growable (not just bounds) so
  `[Min(10), Fill(1)]` means "at least 10, share the rest evenly", the common
  intent; use `Length` for exact.

## Testing

Unit tests for each rule and edge (empty constraint list, zero axis, oversize
gaps, caps, alignment, weight 0, sidebar example) and a randomised property
test (20,000 cases, fixed seed, no new dependency) asserting: one rectangle per
constraint; deterministic; all inside the area; ordered and non-overlapping;
cross axis exact; `Max`/`Length` never exceeded; `Length` exact when there is
room. A facade test feeds the rectangles into RFC-0004's `Renderer` as paint
clips and damage.

## Prior art

Ratatui `Layout`/`Constraint` (the vocabulary names `Length`, `Percentage`,
`Min`, `Max`, `Fill` follow that lineage; the solver here is not Cassowary-based
and the rules are written out so they are testable), CSS flexbox (weights and
caps), Bubble Tea's Lip Gloss (cell-first sizing).

## Non-goals (this slice)

Grid; the measure/arrange pass; nested containers and keyed children; padding
and margin beyond `Rect::inner`; scrolling; invalidation (none / repaint /
layout) wiring; any text measurement.

## Alternatives considered

1. **Cassowary / constraint solver.** Powerful but non-local, hard to document
   as a rule, and slow to reason about when it picks among equal solutions.
2. **Taffy now.** Float/CSS units and its own rounding; ADR-0003 says it may
   come later behind this seam. Not needed for the alpha vocabulary.
3. **Proportional overflow shrinking.** See "Why these choices".
4. **Float weights.** Rejected; integer weights keep results exact and
   snapshot-stable.

## Open questions

1. Whether the alpha needs `Ratio(a, b)` or `SpaceBetween` alignment.
2. Minimum terminal size policy (RFC-1 Open Q2): this RFC only fixes
   per-container overflow; the app-level "terminal too small" behaviour belongs
   to the lifecycle RFC.
3. Grid track syntax and spanning.
4. How measured content sizes enter the solver (`Fit`/intrinsic constraints):
   depends on the RFC-2 component contract and text measurement.
5. Cache keys for layout results (invalidation slice).

## Drawbacks

- New public vocabulary to maintain. `#[non_exhaustive]` makes additions
  non-breaking for callers, but every variant still costs documentation, tests,
  and a rule in the solver.
- "Later items yield first" can surprise authors who expect equal shrinkage; it
  is documented and tested, not hidden.
