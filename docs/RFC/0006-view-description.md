# RFC-0006: View Description and Paint Pass

- Status: **Draft — implemented on branch `phase4-layout-solver`; awaiting a
  maintainer decision** (`docs/RFC/README.md`). Nothing here is Accepted until
  the maintainer records it in `docs/reviews/RFC_DECISION_TRACKER.md`.
- Author(s): Phase 4 implementation (Claude, for @hamroqulovv)
- Created: 2026-10-17
- Related: `docs/research/RENDERING_MODEL_RESOLUTION.md` (§5 pipeline C, §8.1
  "exact `Description` encoding … needs the renderer RFC first"),
  `docs/RFC/0002-component-model.md` (§1 rule 2, §8), `docs/RFC/0004-rendering-engine.md`,
  `docs/RFC/0005-layout-solver.md`, `docs/API_DESIGN.md` (§1, §4)

## Summary

`jilva-core::view` adds `View`: an **owned, geometry-free frame description**,
and `View::paint(area, &mut Buffer)`, the framework-side pass that runs layout
over it and paints cells. `Renderer::render(&view, &caps)` chains describe →
paint → diff for immediate-mode use. This is the *static* half of the component
model; identity, state slots, events, and the `Component` trait are not here.

## Motivation

`RENDERING_MODEL_RESOLUTION.md` fixed the pipeline (user code returns an owned
description; the framework lays out and paints into a reused buffer; `Frame`
never appears in the `Component` trait) but deferred the description's encoding
until the buffer, diff, and damage semantics existed. RFC-0004 and RFC-0005
now supply them, so the encoding can be decided against real consumers rather
than guessed. It also gives users and tests a way to build and assert a whole
screen today, before the component contract exists.

## Proposal

### Shape

`View` is a public opaque struct over a private node enum — the node set can
grow without breaking callers, and no variant is pattern-matchable by users
(they describe; the framework interprets).

| Constructor / combinator | Meaning |
|---|---|
| `View::empty()` | draws nothing |
| `View::fill(attrs)` | styled blanks over the whole region (background) |
| `View::text(&str)`, `styled_text`, `line([Span])` | text; `'\n'` splits lines; clipped, never wrapped |
| `.text_align(Align)` | per-line start/center/end by cell width |
| `View::row(items)`, `View::column(items)` | `(Constraint, View)` children laid out by RFC-0005; `.gap(n)`, `.align(a)` |
| `View::overlay(layers)` | layers painted in order over the same region |
| `.padding(n)` | inset by `n` via `Rect::inner` |
| `.bordered(style, attrs)`, `.title(&str)` | one-cell frame (`Plain`, `Rounded`, `Double`, `Heavy`) around the child |

`Span` is a text run with one `ResolvedAttributes`. Attributes are *resolved*
values: semantic tokens and themes (RFC-3) resolve before this layer, and a
future token-aware constructor sits above it.

### Rules

1. **Owned and pure.** A `View` has no lifetimes, no coordinates, no terminal;
   it is `Clone + Eq`, so golden tests assert descriptions directly.
2. **Paint is infallible and clips.** Every write is clipped to the region and
   the buffer; empty regions draw nothing; over-constrained layouts yield empty
   child regions (RFC-0005 rule 5). There is no error path after description, so
   there is no partially painted state to define (resolution §5 rule 5).
3. **Deterministic and idempotent.** Same view, same area ⇒ same cells;
   painting twice changes nothing more.
4. **Contained.** Only cells inside `area` change, apart from the one-cell
   repair of a wide glyph cut by the area's edge (RFC-0004 §3).
5. **Overlay replaces cells it writes.** Later layers do not blend; cells they
   do not write keep the earlier layer. (Blending, i.e. transparency, is out of
   scope.)
6. **Alignment measures with the width authority.** `text_align` uses
   `text_width` (RFC-0004 §4) at paint time. Layout still never measures text:
   containers get their sizes from constraints; only the placement of a line
   *inside its already-laid-out region* looks at width.
7. **Border degrade.** A region under 2×2 draws no border; the child gets the
   (empty) inner region. A title is inset two cells from each corner and clipped.

### `Renderer::render`

`render(&view, &caps)` clears the back buffer, paints over the full screen, and
presents with `Damage::Full`. Only changed cells cross the seam (the diff
compares against the front buffer), but every cell is repainted and compared.
Retained-mode callers keep using `back_buffer_mut` + `present(&damage)`.

## Testing

Unit tests for each constructor and edge (alignment with wide text, spans,
nested stacks, overlay order, padding/border degradation, empty/zero areas,
containment of writes outside the area, idempotence) plus a randomised test
(3,000 trees × random areas, fixed seed): painting never panics and the buffer's
wide-glyph pairing invariant always holds. Facade tests: a described screen
reaches the headless backend and only the changed digit follows.

## Prior art

Ratatui widgets render into a buffer directly (the shape this project's
resolution rejected for user code); Bubble Tea's `View() string` and Textual's
widget tree return descriptions the framework interprets, which this follows.

## Non-goals

Text wrapping and ellipsis; scrolling and viewports; transparency; hit-testing
and focus; intrinsic (content-measured) sizing; grid; semantic tokens and
themes; the `Component` trait, identity, state, events, change classes.

## Alternatives considered

1. **Public enum of nodes.** Lets users pattern-match and freezes the node set.
   Rejected; builders are the contract.
2. **User code paints (`&mut Frame`).** Already rejected by the resolution
   document (B1/B2).
3. **Style by semantic token inside `View`.** Needs RFC-3's resolver and a
   context parameter; a thin layer above this one, deferred to the design-system
   phase.
4. **Box-model `Block` node with border + padding + background in one.**
   Composition (`overlay`, `padding`, `bordered`) covers it with fewer rules.

## Open questions

1. Text wrapping and truncation-with-ellipsis (needs a break authority and a
   decision about span handling).
2. Intrinsic sizing: how a `View` reports a measured size to the solver
   (RFC-0005 open question 4) without layout shaping text.
3. Whether `Overlay` needs z-order/clip semantics beyond paint order.
4. Keyed children: identity metadata on stack items arrives with the component
   contract; the `(Constraint, View)` pair is expected to grow a key slot.
5. A diff on `View`s (skip repaint of unchanged subtrees) — pairs with change
   classes (RFC-2 §5).

## Drawbacks

- Another public type family to maintain pre-0.1.
- Immediate-mode `render` repaints everything each call; cheap for terminal
  sizes (see RFC-0004 numbers) but not free. Damage-driven repaint is the
  retained-mode path.
