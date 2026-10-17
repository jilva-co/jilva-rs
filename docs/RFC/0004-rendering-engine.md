# RFC-0004: Rendering Engine — Cell Buffer, Diff, Damage

- Status: **Draft — implemented on branch `phase3-rendering-engine`; awaiting a
  maintainer decision** (`docs/RFC/README.md`). Nothing here is Accepted until
  the maintainer records it in `docs/reviews/RFC_DECISION_TRACKER.md`.
- Author(s): Phase 3 implementation (Claude, for @hamroqulovv)
- Created: 2026-10-17
- Related: `docs/ARCHITECTURE.md` (§3.7, §8.1–§8.4, §8.10, §8.11, §8.13),
  `docs/RFC/0001-framework-scope.md` (Unicode row, Open Q12),
  `docs/RFC/0002-component-model.md` (§8 render pipeline),
  `docs/research/RENDERING_MODEL_RESOLUTION.md` (pipeline C: framework-owned,
  reused cell buffer), `docs/PHASE2_BACKEND_CONTRACT.md` (`OutputCommand`),
  `docs/ROADMAP.md` (Phase 3)

## Summary

`jilva-core` gains a rendering engine that is a **pure function** of
`(previous buffer, next buffer, damage, capability profile, options)` and
returns a `Vec<OutputCommand>`:

- **`Cell`** holds one grapheme cluster, its resolved attributes, and its column
  width. It replaces the provisional `Cell { symbol: char }` (RFC-1 Open Q12).
- **`Buffer`** is an owned grid that keeps double-width glyphs whole by
  construction.
- **`diff` / `redraw`** compare buffers and emit the smallest sensible stream:
  coalesced segments, tracked attributes, minimised cursor moves.
- **`Damage`** is the caller's promise of which cells may have changed, so a
  small change in a large static screen is not re-scanned or re-emitted.
- **`Renderer`** is a double buffer that wraps `diff` with invalidation.

No terminal I/O, no globals, no async. The component model, layout, and theming
are separate phases; this layer consumes already-resolved attributes and
already-placed text.

## Motivation

Phase 1 left one placeholder in the renderer module (`Cell { symbol: char }`)
and a roadmap obligation: replace it with a grapheme-capable type, name the
width and segmentation authorities, and define empty-vs-styled-blank and
continuation semantics *before* golden tests entrench a `char`-shaped API.
Phase 2 delivered the backend seam and `OutputCommand`; nothing yet produces
those commands from a frame. This RFC closes that gap and answers
`ARCHITECTURE.md` §8.1–§8.4, §8.10 and §8.11.

## Proposal

### 1. Pipeline position

```text
paint ─► back Buffer ─► diff(front, back, damage, caps, options) ─► Vec<OutputCommand> ─► Backend::submit
                  ▲               │
                  └── front Buffer ◄── synced for the damaged spans after present
```

Consistent with `RENDERING_MODEL_RESOLUTION.md` §5: `Buffer` is framework-owned
and reused; component code never names it. `paint` into it is infallible and
clips instead of failing.

### 2. Ownership and bounds (§8.1)

A `Buffer` owns its cells; there are no views or lifetimes. Every write clips to
the buffer (and an optional clip rectangle) and never panics. Reads outside the
grid return `None`. A zero-sized buffer is valid and inert (the backend refuses
`(0, 0)` sizes, so this only matters for tests and degenerate resizes).

### 3. The cell (§8.1, RFC-1 Q12)

| Width | Kind | Symbol |
|---|---|---|
| 1 | narrow cell | one grapheme cluster |
| 2 | wide head | one double-width cluster |
| 0 | continuation | empty; right half of the head to its left |

- **Symbol type.** Private `Symbol`: up to 15 bytes inline, otherwise
  `Box<str>`. Canonical (inline iff it fits), so equality is a direct
  representation compare. Inline covers ASCII, base+combining, flags, and
  emoji with a variation selector or ZWJ pair; long ZWJ sequences (a family
  emoji is 25 bytes) spill to the heap. No third-party string crate.
- **Pairing invariant.** Every wide head is followed by exactly one
  continuation and every continuation follows a head. Writes *repair* what they
  overwrite: replacing half of a wide glyph blanks the other half, keeping its
  attributes. A torn glyph is unrepresentable, so the diff never has to reason
  about one.
- **Empty versus styled blank.** The *ground state* is a space with plain
  attributes — what a freshly cleared terminal shows. A new buffer is all
  ground state, so the first diff against it emits only content. A space with a
  background colour is a different cell and is emitted; erasing a filled region
  back to ground state emits spaces. `Cell::is_blank` (any attributes) and
  `Cell::is_ground_state` (plain) keep the two questions separate.
- **Attributes** are `ResolvedAttributes` (already core-owned, `Copy`, `Hash`).
  The buffer stores them *un-downgraded*; downgrade happens at diff time against
  the explicit capability profile, so one buffer can be presented to different
  tiers and a tier change needs no repaint of the buffer.

### 4. Width and segmentation authorities (§8.3, §8.4)

| Question | Authority | Configuration |
|---|---|---|
| Where does a character end? | `unicode-segmentation` 1.13 extended grapheme clusters (UAX #29) | defaults |
| How many cells wide is a cluster? | `unicode-width` 0.2 (UAX #11), clamped to `0..=2` | `default-features = false`: **no `cjk` feature**, East-Asian-Ambiguous = 1 |

Both are pure data-table crates with no I/O and no transitive dependencies; the
justification required by `ARCHITECTURE.md` §6.6 is that hand-maintaining
Unicode tables is a correctness liability this project should not own. They are
confined to `renderer/width.rs`; nothing else in the workspace names them.

Rules, all tested:

- A cluster containing any control (C0, DEL, C1) is **skipped** by every text
  writer, never drawn. Controls therefore cannot reach `PrintSegment`, in
  addition to the backend's own validation.
- Zero-width clusters (lone combining marks, ZWSP) are skipped. They are not
  attached to a neighbouring cell: attaching mutates a cell the caller did not
  address, which would break damage reasoning.
- **Truncation never splits a cluster or a wide cell.** A wide glyph with one
  column of room is replaced by one styled blank so the row stays visually
  consistent; a wide glyph straddling the *left* clip edge leaves one blank at
  the edge. Text is never wrapped here (wrapping is layout's job).
- **Terminal disagreement (fallback).** The renderer trusts its table. If a
  terminal's width table differs, the damage is one glyph, not the rest of the
  row, when `DiffOptions::with_resync_after_wide(true)` is set: a cursor move
  follows every double-width glyph. It is off by default because it costs bytes
  on CJK-heavy output. The renderer does not probe the terminal mid-frame
  (§8.14).

### 5. Diff (§8.2)

`diff` compares cell-by-cell within the damaged spans of each row.

- **Effective equality.** Cells are equal when the symbol matches and the
  attributes are equal *after* downgrade to the capability profile (colour
  tier, and attribute support such as italic at 16 colours). A change the
  terminal cannot show emits nothing. A cheap raw-equality check comes first.
- **Continuation cells are never "changed" themselves**; their head carries
  them. The pairing invariant guarantees a head change is always seen first.
- **Coalescing.** Adjacent changed cells with equal effective attributes become
  one `PrintSegment`. `SetAttributes` is emitted only on change; returning to
  plain emits `ResetAttributes`.
- **Cursor minimisation.** The emitter tracks the cursor and omits `MoveTo` when
  the next cell is where the cursor already is. After the last column the
  cursor is forgotten (pending-wrap is not portable). Short gaps of unchanged
  cells (`bridge_gap`, default 4 columns) are printed through rather than
  skipped with a move, when that adds text but no attribute switch the stream
  would not need anyway (the gap carries the current attributes or those of the
  next changed cell). Bridging is disabled with `resync_after_wide`.
- **Stream contract** (module docs): a stream assumes default attributes and an
  unknown cursor at its start, ends with default attributes, validates for the
  buffer size, and is empty when nothing visible changed. Streams are therefore
  independent of earlier streams.
- **Size mismatch.** If the two buffers differ in size, `previous` says nothing
  about the screen: `redraw` is used (`ResetAttributes`, `ClearRegion` of the
  whole screen, then every non-ground cell).
- **Worst case.** Full-screen churn is linear in cells; the stream length is
  bounded by (rows × runs-per-row) commands, not by cells (tested: 120×40 where
  every cell changes emits exactly one `MoveTo` and one segment per row). No
  quadratic path: the look-ahead for the next changed cell resumes the outer scan
  where it stops.

`ClearRegion` is used only by `redraw`, after a `ResetAttributes`, because the
real backend implements it by printing spaces in the *current* attributes; the
diff never relies on it for styled blanks.

### 6. Damage tracking (§8.10)

`Damage` is `Full` (default) or `Regions(Vec<Rect>)`. It is a **promise**: cells
outside the damage are identical in both buffers and are not compared.
Under-reporting leaves stale cells; over-reporting only costs scan time.

- Rectangles may overlap, touch, be empty, or extend past the buffer; they are
  clipped, sorted, and merged per row in place.
- **Wide-glyph widening.** A span that begins on a continuation or ends after a
  wide head — in *either* buffer — is widened by one column. This is what makes
  clipped painting safe: painting inside a rectangle can blank the other half of
  a glyph that straddles its edge, and that neighbour is still covered.
  `Renderer` uses the same function to sync the front buffer, so the two never
  diverge.
- Damage flows from the caller; this RFC does not decide how components derive
  it (Phase 4/5: change classes `none | repaint | layout`, RFC-2 §5). The
  rectangle list is the narrow seam they will feed.

### 7. `Renderer`

Double buffer: paint into `back_buffer_mut()`, call `present(&damage, &caps)`,
submit the returned stream. After `present` the back buffer keeps its content
(retained mode); immediate-mode callers `clear()` it first.

The front buffer is a *belief* about the screen. A full redraw is scheduled
automatically on construction, `resize`, and a change of `caps.color` between
presents (downgrade changes what every cell looks like). The caller calls
`invalidate()` after a failed `submit`/`flush`, a suspend/resume, or any external
write to the screen. Damage is ignored when a redraw is due.

### 8. Testing (§8.13)

- Unit: width table cases; cell kinds; every buffer operation and its edge cases;
  golden streams for each diff rule.
- **Round-trip property** (1,500 randomised rounds + 300 redraw rounds, fixed
  seeds, no new dependency): a small virtual terminal interprets the stream; the
  screen after `apply(diff(prev, next))` must equal `next` as the terminal can
  show it, across all four colour tiers, four option sets, random damage with
  clipped painting, wide/ZWJ/flag/combining text, styled blanks, and stream
  validity for the screen size. Unchanged input must yield an empty stream.
- **Invariant property:** thousands of random writes/fills/restyles never break
  the pairing invariant.
- Integration through the facade only (`tests/render_pipeline.rs`): golden
  frames against `TestDoubleBackend`, resize, oversize-frame refusal, hostile
  text.

## Performance posture (§8.11)

`benches/render_diff.rs` (std only, `harness = false`) on a 200×60 screen of
mixed narrow/wide/styled text. One run, release profile, one Windows machine:

| Scenario | Time per frame | Commands |
|---|---|---|
| unchanged, `Damage::Full` | ≈ 86 µs | 0 |
| one cell changed, `Damage::Full` | ≈ 86 µs | 6 |
| one cell changed, one-cell damage region | ≈ 0.3 µs | 6 |
| full-screen churn, every cell restyled | ≈ 530 µs | 181 |

Reading: scanning is roughly 7 ns/cell; damage reduces a one-cell change from
scanning 12,000 cells to touching one, a ~300× difference *for this scenario*.
These are ordering numbers, not a public claim: single machine, no variance
analysis, no comparison against other frameworks. Allocation in the diff is
the output `Vec`, one text `String` per emitted segment, and a small per-diff
span `Vec`; the buffers themselves are not allocated per frame.

## Prior art

Ratatui's `Buffer`/`diff` (cell grid, previous/current swap, skipping the cells
covered by a wide symbol) and Bubble Tea's line-based renderer informed the
shape. The differences here are deliberate: symbols are clusters rather than
strings-per-cell with ad hoc width, equality is capability-effective, damage is
an explicit input, and the stream is self-contained so a failed flush cannot
corrupt later streams' assumptions.

## Non-goals

Layout, text wrapping, hyperlinks (OSC 8), images, synchronized-output mode,
scroll regions, and cursor ownership/visibility policy (RFC for lifecycle/events).
Run-length compression of repeated cells (see Open questions).

## Alternatives considered

1. **Keep `char` per cell.** Rejected: cannot represent combining marks, flags,
   or ZWJ emoji; the roadmap forbids entrenching it.
2. **`String` per cell.** Correct but allocates for every cell and every clone;
   the inline form avoids that for the overwhelming majority of cells.
3. **Attach zero-width clusters to the previous cell.** Rejected (§4): mutates a
   cell the caller did not address.
4. **Automatic damage from buffer writes.** Rejected for now: the double-buffer
   paint model repaints regions wholesale, so write-tracking would mark the whole
   region damaged anyway; damage belongs with the component change classes.
5. **Wide glyph = one cell with `width = 2` and no continuation.** Rejected: the
   diff, clipping and overlap repair would all need special cases for "the
   column a glyph covers but does not occupy"; the continuation makes every
   column addressable.
6. **Use `ClearRegion` for styled blank runs.** Rejected: backend semantics are
   "spaces in current attributes"; printing them is equivalent and explicit.

## Open questions

1. **Run-length compression** (`ECH`/`REP`) for long runs of identical cells —
   needs new additive `OutputCommand` variants and a backend decision.
2. **Synchronized output** (DEC mode 2026) to avoid tearing on large updates —
   lifecycle/capability RFC.
3. **Real-terminal width disagreement.** The fallback is a documented option, not
   a probe. Is an opt-in startup probe worth its complexity?
4. **Benches in CI.** The bench exists and is run manually; a regression gate
   needs stable hardware.
5. **Cell size.** A cell is larger than necessary (attribute flags are six
   `bool`s). Packing is a measurable-win-only change and is deferred.
6. **Alt-width option.** Whether to expose the `cjk` width variant as an opt-in
   for East-Asian locales.

## Drawbacks

- Two new third-party dependencies in `jilva-core` (justified in §4).
- `Cell::new(char)` and the public `symbol` field are gone; the API was
  `0.0.0` and provisional, but it is a break.
- `Damage` is only as honest as its caller; a wrong promise shows up as stale
  cells, not an error.
