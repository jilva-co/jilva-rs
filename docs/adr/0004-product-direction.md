# ADR-0004: Product direction — beauty first, hybrid state, dashboard dogfood

- Status: Accepted by the project owner in conversation on 2026-10-17
  (recorded here at the owner's direction; not an RFC decision).
- Related: `README.md` (Philosophy, "taste" bet), `docs/RFC/0002-component-model.md`,
  `docs/RFC/0007-motion-and-blending.md`

## Context

Jilva must be distinguishable from existing Rust TUI frameworks, not a
re-implementation. The owner asked that it be better, faster, and more
beautiful. The project's own rules require that no superiority claim appear
without falsifiable evidence (`docs/ARCHITECTURE.md` §9).

## Decision

1. **Headline: beauty.** Defaults, motion, and colour carry the identity.
   Speed and simplicity are requirements, not the pitch.
2. **First visual features: animation/easing and colour blending/gradients**
   (RFC-0007). Curated themes (RFC-3 tokens), images, and charts follow.
3. **State model: hybrid.** Application state is Elm-shaped (model, messages,
   `update`, pure `view`) per RFC-2; widget-local ephemeral state (scroll,
   cursor, open/closed, animation progress) lives in framework-owned slots keyed
   by component identity. Fine-grained reactive signals are not adopted:
   they would rewrite accepted RFC-2, and the change classes
   (`none`/`repaint`/`layout`) plus renderer damage already bound the work done
   per event.
4. **First dogfood application: a dashboard** (table, chart, tabs, status bar).
   It exercises layout, damage, colour tiers, and motion together, and yields
   the screenshots and numbers below.

## Making the claims falsifiable

- *Faster:* `benches/` carries a reproducible diff/paint benchmark
  (`render_diff`); a like-for-like comparison with another framework on the same
  scene and machine is published with its method before any comparative claim
  is made in the README.
- *More beautiful:* a widget and theme gallery with truecolor / 256 / 16 / mono
  captures, plus the dashboard, are the evidence; "beautiful" is not argued in
  prose.
- *Better:* an API-size and example-length comparison on the dashboard, with
  the sources linked.

## Consequences

- Phase 4 continues with the component contract and the dashboard widgets,
  prioritised by what the dashboard needs.
- RFC-3 theming moves earlier because curated themes are headline work.
- Anything that would add wall-clock reads, timers, or async to `jilva-core` is
  rejected: motion takes injected time.
