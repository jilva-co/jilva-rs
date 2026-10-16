# ADR-0003: Terminal backend strategy (Crossterm default behind a decoupled seam)

- Status: Accepted
- Date: 2026-10-16
- Context: Phase 1 foundation, `phase1-foundation` branch.
- Related: `docs/ARCHITECTURE.md` (§3.5, §3.10, §8.14), `docs/API_DESIGN.md` (§9).

## Context

Every TUI framework must answer: how do bytes get to and from the terminal,
and how tightly is that mechanism woven into the framework? Candidates for the
initial backend include Crossterm (pure-Rust, cross-platform), termwiz, and
direct libc/Win32 Console handling. A second, subtler question: what may the
backend *know* — raw instruction streams only, or components, state, and styles?
And a third, forward-looking one: where would a layout engine like Taffy fit,
given it speaks CSS pixels while terminals speak integer cells?

## Decision

Three linked decisions:

1. **Crossterm is the planned initial backend**, implemented inside
   `jilva-terminal`, chosen for pure-Rust cross-platform support (Unix +
   Windows) without native-UI frameworks, matching our platform matrix with
   the least platform-specific code of our own. This is *planned*, not
   hard-wired: the version choice belongs to the implementation RFC/PR, and
   swapping backends later must not touch `jilva-core` or user code beyond
   the construction edge.
2. **The backend seam is decoupled from app state by construction.**
   `jilva-terminal` receives fully-resolved draw instructions and emits decoded
   events; it takes no component, application-state, or style-token types as
   inputs and returns none as outputs. Capability *detection* lives here;
   capability *profiles* cross into core as plain data. Rationale: backends
   change for platform reasons, app logic changes for product reasons — a
   shared type would couple unrelated rates of change and make headless testing
   (test double substitution) a refactor instead of a one-line construction
   choice.
3. **Taffy (or any CSS-layout engine) may integrate later, strictly behind the
   layout seam, with an explicit unit translation.** CSS layout operates in a
   continuous, pixel-oriented box model (fractional flex, content-based sizing);
   terminal layout operates on **integer cell rectangles** where text width is
   measured in grapheme cells including double-width characters. A future
   `jilva-layout` extraction would therefore wrap the engine with an adapter:
   shape text first (graphemes/widths → concrete cell measurements), feed the
   engine cell-quantised constraints, then snap its fractional results back to
   non-overlapping integer rectangles under the documented remainder policy.
   Component code must never observe CSS units, floats, or engine internals —
   only cell regions. Whether Taffy is adopted at all is a future RFC decision;
   this ADR only guarantees the seam admits it without redesign.

## Consequences

- Positive: backend upgrades, bug workarounds, and platform fixes stay inside
  one crate; core and user code are insulated. Headless golden-output testing
  is structural, not a special build mode.
- Positive: the CSS-vs-cell distinction is named before any code exists, so no
  RFC or prototype can accidentally leak fractional coordinates into the
  component model.
- Negative: maintaining our own seam over Crossterm adds a thin abstraction
  layer with its own bugs and docs; accepted because backend-swappability and
  testability outweigh the cost, and the seam is deliberately narrow.
- Negative: Crossterm feature coverage (progressive key encodings, mouse modes)
  bounds what we offer on day one; gaps are handled by the documented encoding
  ladder and degradation policy (ARCHITECTURE.md considerations §8.6, §8.14),
  not by backend-specific hacks in core.

## Alternatives considered

1. **termwiz as the initial backend.** Respected alternative (rich model,
   strong Windows story). Deferred, not dismissed: Crossterm is chosen for the
   smaller, more conservative initial surface and wider current familiarity,
   which matters for contributor onboarding pre-`0.1.0`. The seam makes this
   choice reversible; a future RFC may re-evaluate with prototype evidence.
2. **Hand-rolled Unix/Windows console I/O, no backend dependency.** Rejected:
   maximum control, but we would own every terminal quirk, capability probe,
   and Windows-console edge ourselves, before shipping anything. Poor trade at
   the foundation stage; reconsider only if a backend proves unfixably limiting.
3. **Backend-aware core (components/styles visible to the backend for
   "optimised" output).** Rejected: short-term optimisation at the price of
   permanent coupling — every backend change would ripple into app-facing
   types, and test doubles would need to replicate app semantics. Optimisation
   belongs in the diff/patch layer operating on instruction streams (§3.7).
4. **Taffy directly as the layout API (CSS units user-visible).** Rejected:
   exposing a pixel-model API for a cell-addressed medium guarantees a class
   of rounding/overlap bugs in user code and ties our public API to an external
   engine version. The adapter-behind-the-seam approach keeps our options open
   and our units honest.
