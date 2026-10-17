# RFC-0007: Motion and Colour Blending

- Status: **Draft — implemented on branch `phase4-motion-blend`; awaiting a
  maintainer decision** (`docs/RFC/README.md`). Nothing here is Accepted until
  the maintainer records it in `docs/reviews/RFC_DECISION_TRACKER.md`.
- Author(s): Phase 4 implementation (Claude, for @hamroqulovv)
- Created: 2026-10-17
- Related: `docs/adr/0004-product-direction.md` (why this comes first),
  `docs/RFC/0004-rendering-engine.md` (effective equality, downgrade at diff),
  `docs/RFC/0006-view-description.md`, `docs/RFC/0002-component-model.md`
  (§4 injected `Tick(now)`, §5 change classes), `docs/RFC/0003-theming-architecture.md`
  (reduced motion, capability tiers)

## Summary

Two small, pure building blocks for "beautiful by default":

1. **Colour blending.** `ResolvedColor::{to_rgb, blend}`, `Buffer::map_attrs`,
   and `View::{gradient, gradient_text, tint}` with a `Backdrop`.
2. **Motion.** `Easing`, `Lerp`, and `Transition<T>`: time-driven values that
   are a pure function of an **injected** `Duration`, with `retarget` for
   interruption and a reduced-motion switch.

Neither adds a clock, timer, thread, or async dependency. Neither changes the
renderer: blended and animated colours are ordinary truecolor attributes that
RFC-0004's diff already downgrades per terminal tier and compares by what the
terminal can show.

## Motivation

ADR-0004 makes beauty the headline and names animation and blending as the
first visual features. The existing overlay *replaces* cells, so a scrim, a
shadow, a glass panel, or a fade is impossible; and nothing models time, so
animation would otherwise be reinvented per widget with clocks hidden inside.
The renderer's capability-effective diff makes both cheap to add correctly:
an animation frame the terminal cannot distinguish costs zero bytes.

## Proposal

### Colour blending

- `ResolvedColor::to_rgb(default)` converts any colour to sRGB-ish: `Rgb`
  as-is, `Ansi16` via the table `downgrade` already uses, `Palette` via the
  xterm layout (16 ANSI, 6×6×6 cube at 0/95/135/175/215/255, 24-step grey
  ramp), `Default` via the caller-supplied stand-in.
- `ResolvedColor::blend(top, alpha, default)` returns `Rgb`. Integer
  round-to-nearest `(top·a + under·(255−a) + 127) / 255` — exact at `a = 0`
  and `a = 255`, identical on every platform, no floats.
- **Honesty about unknowns.** The terminal default and the first 16 colours are
  user-configurable and unknowable from inside the process. Blending against
  them uses a documented stand-in (`Backdrop::dark()` / `light()` / `new`), and
  the result is an explicit `Rgb` — so a blended UI is accurate *to the
  assumption*, not to the user's theme. A theme (RFC-3) supplies the real
  backdrop; until then the assumption is visible in the API, not hidden.
- `Buffer::map_attrs(rect, f)` applies a pure function to the attributes of
  every cell in a rectangle, widening to whole wide glyphs (so head and
  continuation cannot disagree). `set_attrs` is now `map_attrs(|_| attrs)`.
- `View::tint(color, alpha, backdrop)`: blends fg and bg of whatever is
  already painted in the region; symbols and decorations stay; `alpha == 0` is
  a true no-op (does not even convert default colours to explicit `Rgb`).
  Layer it last in an `overlay` for scrims, dimming, glass.
- `View::gradient(from, to, Direction)`: a background gradient, one step per
  column (`Row`) or row (`Column`), inclusive of both endpoints.
- `View::gradient_text(text, from, to)`: per-grapheme foreground gradient,
  one gradient per line.
- No transparency *inside* the cell model: a cell stays one symbol with one fg
  and one bg. Blending is an operation on what has been painted, performed at
  paint time. That keeps the diff and the pairing invariant untouched.

### Motion

- `Easing::{Linear, EaseIn, EaseOut, EaseInOut, EaseOutBack}`. Cubic curves;
  `apply` clamps input to `0..=1` (NaN → 0), maps 0→0 and 1→1; only
  `EaseOutBack` leaves the range in between (≈ +10 % overshoot).
- `Lerp` for `f32`, `i32` (rounded to a cell), `(u8, u8, u8)` and
  `ResolvedColor`. Truecolor pairs interpolate; any other pairing **steps** at
  `t ≥ 0.5` because a default/palette/ANSI colour has no midpoint without the
  terminal's palette. Overshoot saturates in colour channels.
- `Transition<T>`: `from`, `to`, `start`, `duration`, `easing`.
  `value_at(now)`, `progress(now)`, `is_finished(now)`, `target()`.
  Sampling is pure and order-independent; before `start` it is `from`, after
  the end it is exactly `to` (no float residue), a zero duration is complete at
  `start`.
- `retarget(now, to, duration)` starts the new motion from the *current* value
  — interrupting an animation never jumps.
- `value_at_reduced(now, reduced_motion)` returns `to` at once when
  `TerminalCapabilities::reduced_motion` is set.

### Time is injected

`now` is a `Duration` from an epoch the caller picks (RFC-2 §4: `Tick(now)`
carries plain data). Nothing here reads a clock, so:

- tests advance time by hand and get bit-identical frames;
- an app with no running transition has no reason to request frames
  (`is_finished`), preserving zero idle CPU;
- the source of ticks stays a lifecycle/event concern (open question 1).

### Why this is cheap at runtime

A frame is `value_at(now)` → `View` → paint → diff. The diff compares by
effective attributes, so frames in which an animated colour does not change
*as the terminal can show it* emit nothing. Measured in
`tests/render_pipeline.rs`: a 200 ms fade sampled every 10 ms produces output
on at least 15 of 20 frames at truecolor, strictly fewer at 16 colours (most
steps collapse to one palette entry), and none after the first frame in mono.
That is a property test of the mechanism, not a performance benchmark.

## Testing

Unit tests: easing endpoints/monotonicity/overshoot; `Lerp` incl. saturation
and stepping; transition sampling before/during/after, zero duration,
settled, reduced motion, retarget continuity, a frame-exact colour fade;
`to_rgb` against the xterm layout; `blend` exactness, identity, monotonicity;
gradient direction/degenerate sizes; tint on fg/bg/default cells, alpha-0
no-op, wide-glyph integrity; gradient text per line; the randomised view paint
test now includes gradients and tints (wide-glyph invariant must hold).
Facade: the fade test above. Doctests on every public item.

## Prior art

CSS transitions and `requestAnimationFrame` timestamps (injected time),
Framer Motion / Flutter `AnimationController` (retargeting from the current
value), Textual's animators, notcurses' alpha model. This proposal keeps time
out of the library and blending out of the cell.

## Non-goals

A scheduler or tick source; spring physics; keyframe timelines and sequencing;
per-cell alpha stored in the buffer; HSL/OKLab interpolation (sRGB linear
interpolation is the first cut; perceptual spaces are an open question);
shadows as a first-class node (compose `tint` with an offset region).

## Alternatives considered

1. **Alpha channel on `ResolvedColor`.** Would force every consumer (diff,
   downgrade, backend) to define compositing and leak into the pairing
   invariant. Rejected: blend at paint time.
2. **Animator objects holding a clock.** Hidden state and hidden time make
   tests and idle behaviour worse. Rejected.
3. **Float colours / linear-light blending.** More correct for gradients, but
   non-integer maths in the hot path and platform-sensitive rounding. Deferred
   (open question 2).
4. **Always interpolate non-truecolor pairs via an assumed palette.** Hides a
   guess inside `Lerp`. Rejected in favour of an explicit step.

## Open questions

1. The tick source and frame pacing (when does the loop wake to sample a
   running `Transition`?) — lifecycle/event RFC.
2. Perceptual interpolation (linear-light or OKLab) for gradients and fades.
3. A `Timeline` for sequencing and staggering transitions.
4. First-class shadow and glass nodes once the dashboard exercises them.
5. Whether `Backdrop` should be derived from `BackgroundHint` automatically
   when the theme layer lands.

## Drawbacks

- sRGB-space blending is visibly imperfect for dark gradients.
- Blending against an assumed default can look wrong on unusual themes until
  RFC-3 supplies real backdrops.
- Floating point in `Easing`/`Transition` (f32): deterministic per platform,
  but golden tests on exact values must tolerate ULP-level differences across
  architectures; colour outputs are rounded to integers to contain this.
